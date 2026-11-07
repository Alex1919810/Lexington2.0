use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct City {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}
impl City {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 300
            || self.name.chars().any(char::is_control)
            || !self.latitude.is_finite()
            || !(-90. ..=90.).contains(&self.latitude)
            || !self.longitude.is_finite()
            || !(-180. ..=180.).contains(&self.longitude)
            || self.timezone.is_empty()
            || self.timezone.len() > 100
            || self.timezone.chars().any(char::is_control)
        {
            return Err("天气城市配置无效，请重新选择城市。".into());
        }
        Ok(())
    }
}

pub enum Job {
    Search(u64, String),
    Forecast(u64, City, String),
    Stop,
}
pub enum Event {
    Cities(u64, Result<Vec<City>, String>),
    Forecast(u64, Result<String, String>),
}
pub struct Client {
    sender: SyncSender<Job>,
    events: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
}
impl Client {
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        Self::with_endpoints(
            "https://geocoding-api.open-meteo.com/v1/search".into(),
            "https://api.open-meteo.com/v1/forecast".into(),
            Duration::from_secs(10),
            wake,
        )
    }
    fn with_endpoints(
        search_url: String,
        forecast_url: String,
        timeout: Duration,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, jobs) = mpsc::sync_channel(4);
        let (events, receiver) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let stopped = stopping.clone();
        let worker = std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout(timeout)
                .redirects(0)
                .build();
            let mut cached: Option<(City, String, Instant, String)> = None;
            while let Ok(job) = jobs.recv() {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                let event = match job {
                    Job::Stop => break,
                    Job::Search(id, query) => {
                        let result = fetch(
                            &agent,
                            &search_url,
                            &[
                                ("name", query.trim().to_owned()),
                                ("count", "8".into()),
                                ("language", "zh".into()),
                                ("format", "json".into()),
                            ],
                        )
                        .and_then(parse_cities);
                        Event::Cities(id, result)
                    }
                    Job::Forecast(id, city, day) => {
                        let matching = cached
                            .as_ref()
                            .filter(|(c, d, _, _)| c == &city && d == &day);
                        let result = if let Some((_, _, when, text)) = matching
                            && when.elapsed() < Duration::from_secs(1800)
                        {
                            Ok(text.clone())
                        } else {
                            city.validate().and_then(|()| fetch(&agent, &forecast_url, &[
                                ("latitude", city.latitude.to_string()),
                                ("longitude", city.longitude.to_string()),
                                ("timezone", city.timezone.clone()),
                                ("temperature_unit", "celsius".into()),
                                ("current", "temperature_2m,apparent_temperature,weather_code".into()),
                                ("daily", "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max".into()),
                                ("forecast_days", "3".into()),
                            ])).and_then(|json| format_forecast(&city, json)).inspect(|text| {
                                cached = Some((city.clone(), day.clone(), Instant::now(), text.clone()));
                            }).or_else(|error| {
                                if let Some((c, d, when, text)) = &cached
                                    && c == &city && d == &day
                                    && when.elapsed() < Duration::from_secs(21600)
                                {
                                    Ok(format!("{text}\n暂时无法更新，以上为上次天气。"))
                                } else { Err(error) }
                            })
                        };
                        Event::Forecast(id, result)
                    }
                };
                if events.send(event).is_err() {
                    break;
                }
                wake();
            }
        });
        Self {
            sender,
            events: receiver,
            worker: Some(worker),
            stopping,
        }
    }
    pub fn send(&self, job: Job) -> Result<(), String> {
        self.sender
            .try_send(job)
            .map_err(|_| "天气服务忙，请稍后重试。".into())
    }
    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        // At most one bounded HTTP request remains; discard queued work on shutdown.
        self.stopping.store(true, Ordering::Relaxed);
        let _ = self.sender.try_send(Job::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn fetch(
    agent: &ureq::Agent,
    url: &str,
    query: &[(&str, String)],
) -> Result<serde_json::Value, String> {
    let mut request = agent.get(url).set("User-Agent", "Lexington/2.0");
    for (key, value) in query {
        request = request.query(key, value);
    }
    let response = request
        .call()
        .map_err(|_| "天气服务连接失败或超时，请稍后重试。".to_owned())?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "读取天气数据失败。".to_owned())?;
    if bytes.len() > 1_048_576 {
        return Err("天气数据过大，已忽略。".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "天气数据格式错误。".into())
}
fn parse_cities(json: serde_json::Value) -> Result<Vec<City>, String> {
    #[derive(Deserialize)]
    struct Place {
        name: String,
        latitude: f64,
        longitude: f64,
        timezone: String,
        #[serde(default)]
        admin1: String,
        #[serde(default)]
        country: String,
    }
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        results: Vec<Place>,
    }
    let response: Response =
        serde_json::from_value(json).map_err(|_| "城市搜索数据格式错误。".to_owned())?;
    response
        .results
        .into_iter()
        .take(8)
        .map(|p| {
            let mut parts = vec![p.name];
            for part in [p.admin1, p.country] {
                if !part.is_empty() && !parts.contains(&part) {
                    parts.push(part);
                }
            }
            let city = City {
                name: parts.join(" · "),
                latitude: p.latitude,
                longitude: p.longitude,
                timezone: p.timezone,
            };
            city.validate()?;
            Ok(city)
        })
        .collect()
}
fn condition(code: u16) -> &'static str {
    match code {
        0 => "晴",
        1 => "晴间多云",
        2 => "多云",
        3 => "阴",
        45 | 48 => "雾",
        51..=57 => "毛毛雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 | 67 => "冻雨",
        71 => "小雪",
        73 => "中雪",
        75 | 77 => "大雪",
        80..=82 => "阵雨",
        85 | 86 => "阵雪",
        95..=99 => "雷雨",
        _ => "未知天气",
    }
}
fn format_forecast(city: &City, json: serde_json::Value) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Current {
        temperature_2m: f64,
        apparent_temperature: f64,
        weather_code: u16,
        time: String,
    }
    #[derive(Deserialize)]
    struct Daily {
        time: Vec<String>,
        weather_code: Vec<u16>,
        temperature_2m_max: Vec<f64>,
        temperature_2m_min: Vec<f64>,
        precipitation_probability_max: Vec<Option<f64>>,
    }
    #[derive(Deserialize)]
    struct Forecast {
        current: Current,
        daily: Daily,
    }
    let data: Forecast =
        serde_json::from_value(json).map_err(|_| "天气数据不完整，请稍后重试。".to_owned())?;
    let current = data.current;
    let daily = data.daily;
    if [
        daily.time.len(),
        daily.weather_code.len(),
        daily.temperature_2m_max.len(),
        daily.temperature_2m_min.len(),
        daily.precipitation_probability_max.len(),
    ]
    .iter()
    .any(|n| *n != 3)
        || !current.temperature_2m.is_finite()
        || !current.apparent_temperature.is_finite()
        || current.time.len() < 16
    {
        return Err("天气数据不完整，请稍后重试。".into());
    }
    let mut text = format!(
        "{}\n{}，当前 {:.0}°C，体感 {:.0}°C。",
        city.name,
        condition(current.weather_code),
        current.temperature_2m,
        current.apparent_temperature
    );
    for (i, label) in ["今天", "明天", "后天"].iter().enumerate() {
        let low = daily.temperature_2m_min[i];
        let high = daily.temperature_2m_max[i];
        let rain = daily.precipitation_probability_max[i];
        if !low.is_finite()
            || !high.is_finite()
            || low > high
            || rain.is_some_and(|p| !p.is_finite() || !(0. ..=100.).contains(&p))
        {
            return Err("天气数据无效，请稍后重试。".into());
        }
        text.push_str(&format!(
            "\n{label} {} {low:.0}～{high:.0}°C",
            condition(daily.weather_code[i])
        ));
        if let Some(rain) = rain {
            text.push_str(&format!("，降水 {rain:.0}%"));
        }
    }
    text.push_str(&format!(
        "\n更新：{}（{}）\n来源：Open-Meteo",
        current.time.replace('T', " "),
        city.timezone
    ));
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{io::Write, net::TcpListener};

    fn city() -> City {
        City {
            name: "上海 · 中国".into(),
            latitude: 31.23,
            longitude: 121.47,
            timezone: "Asia/Shanghai".into(),
        }
    }
    fn forecast() -> serde_json::Value {
        json!({
            "current":{"temperature_2m":22.4,"apparent_temperature":23.1,"weather_code":2,"time":"2026-11-07T14:20"},
            "daily":{"time":["2026-11-07","2026-11-08","2026-11-09"],"weather_code":[2,61,0],
                "temperature_2m_max":[25.,24.,26.],"temperature_2m_min":[19.,18.,20.],"precipitation_probability_max":[30.,80.,null]}
        })
    }
    fn server(responses: Vec<(Duration, String)>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (delay, body) in responses {
                let until = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < until =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("mock HTTP accept: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut part = [0; 1024];
                    let n = stream.read(&mut part).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&part[..n]);
                    if bytes.windows(4).any(|p| p == b"\r\n\r\n") {
                        break;
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                std::thread::sleep(delay);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            }
            requests
        });
        (url, worker)
    }
    fn event(client: &Client) -> Event {
        client
            .events
            .recv_timeout(Duration::from_secs(3))
            .expect("weather event")
    }
    #[test]
    fn weather_parsing_preserves_locations_and_rejects_incomplete_forecasts() {
        let cities = parse_cities(json!({"results":[{"name":"上海","admin1":"上海","country":"中国","latitude":31.23,"longitude":121.47,"timezone":"Asia/Shanghai"}]})).unwrap();
        assert_eq!(cities, vec![city()]);
        assert!(parse_cities(json!({})).unwrap().is_empty());
        assert!(parse_cities(json!({"results":[{"name":"bad"}]})).is_err());
        let text = format_forecast(&city(), forecast()).unwrap();
        assert!(text.contains("当前 22°C，体感 23°C"));
        assert!(text.contains("今天 多云 19～25°C，降水 30%"));
        assert!(text.contains("明天 小雨"));
        assert!(text.contains("后天 晴"));
        assert!(text.contains("2026-11-07 14:20（Asia/Shanghai）"));
        let mut bad = forecast();
        bad["daily"]["temperature_2m_max"] = json!([25]);
        assert!(format_forecast(&city(), bad).is_err());
        let mut bad = forecast();
        bad["daily"]["precipitation_probability_max"][0] = json!(101);
        assert!(format_forecast(&city(), bad).is_err());
        let mut bad = city();
        bad.latitude = f64::NAN;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn weather_http_encodes_queries_caches_same_city_and_refreshes_on_city_change() {
        let (url, server) = server(vec![
            (Duration::ZERO, json!({"results":[]}).to_string()),
            (Duration::ZERO, forecast().to_string()),
            (Duration::ZERO, forecast().to_string()),
        ]);
        let client = Client::with_endpoints(url.clone(), url, Duration::from_secs(1), || {});
        client
            .send(Job::Search(1, "上海 & district".into()))
            .unwrap();
        assert!(matches!(event(&client),Event::Cities(1,Ok(c)) if c.is_empty()));
        for id in [2, 3] {
            client
                .send(Job::Forecast(id, city(), "2026-11-07".into()))
                .unwrap();
            assert!(
                matches!(event(&client),Event::Forecast(i,Ok(text)) if i==id && text.contains("Open-Meteo"))
            );
        }
        let mut other = city();
        other.longitude += 1.;
        client
            .send(Job::Forecast(4, other, "2026-11-07".into()))
            .unwrap();
        assert!(matches!(event(&client), Event::Forecast(4, Ok(_))));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].contains("%26"));
        assert!(requests[1].contains("forecast_days=3"));
        assert!(requests[1].contains("timezone=Asia%2FShanghai"));
    }
    #[test]
    fn weather_timeout_bad_json_and_oversized_responses_return_errors() {
        for (delay, body) in [
            (Duration::from_millis(250), "{}".to_owned()),
            (Duration::ZERO, "invalid".into()),
            (Duration::ZERO, "x".repeat(1_048_577)),
        ] {
            let (url, server) = server(vec![(delay, body)]);
            let client =
                Client::with_endpoints(url.clone(), url, Duration::from_millis(100), || {});
            client.send(Job::Search(1, "Test".into())).unwrap();
            assert!(matches!(event(&client), Event::Cities(1, Err(_))));
            drop(client);
            server.join().unwrap();
        }
    }
    #[test]
    fn weather_settings_save_preserves_keys_and_rejects_invalid_or_broken_config() {
        use crate::backend::config::Config;
        let root = std::env::temp_dir().join(format!(
            "lexington-weather-config-{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(root.join("config")).unwrap();
        let path = root.join("config/config.json");
        std::fs::write(&path, r#"{"unknown":42,"codex":{"apiKey":"fixture-only"}}"#).unwrap();
        Config::save_weather(&root, Some(&city())).unwrap();
        assert_eq!(Config::read(&root).unwrap().weather.city, Some(city()));
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["unknown"], 42);
        assert_eq!(json["codex"]["apiKey"], "fixture-only");
        let before = std::fs::read(&path).unwrap();
        let mut bad = city();
        bad.longitude = 181.;
        assert!(Config::save_weather(&root, Some(&bad)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        Config::save_weather(&root, None).unwrap();
        assert!(Config::read(&root).unwrap().weather.city.is_none());
        std::fs::write(&path, "broken").unwrap();
        assert!(Config::save_weather(&root, Some(&city())).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken");
        std::fs::remove_dir_all(root).unwrap();
    }
}
