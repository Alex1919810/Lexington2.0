//! Offline CPU embedding. No role policy, HTTP requests, model discovery or downloads.
use fastembed::{
    AutoDevicePolicy, InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding,
    TokenizerFiles, UserDefinedEmbeddingModel,
};
use serde::Deserialize;
use std::{fs, path::Path, sync::OnceLock};

pub const MODEL_SPACE: &str = "local:bge-small-zh-v1.5:15b717c382bcb518:cls:single-int8:v1";
pub const MODEL_DIRECTORY: &str = "resource/models/bge-small-zh-v1.5";
pub const CONTENT_TOKENS: usize = 440;
const QUERY_PREFIX: &str = "为这个句子生成表示以用于检索相关文章：";

pub struct LocalEmbedding {
    model: TextEmbedding,
}

#[derive(Deserialize)]
struct Manifest {
    files: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    size: usize,
    sha256: String,
}

fn read_asset(directory: &Path, name: &str) -> Result<Vec<u8>, String> {
    read_verified(
        directory,
        name,
        include_str!("../../resource/models/bge-small-zh-v1.5/manifest.json"),
    )
}

fn read_verified(directory: &Path, name: &str, manifest: &str) -> Result<Vec<u8>, String> {
    let manifest: Manifest = serde_json::from_str(manifest).map_err(|_| "内置模型清单无效。")?;
    let asset = manifest
        .files
        .iter()
        .find(|asset| asset.name == name)
        .ok_or("内置模型文件未登记。")?;
    let bytes = fs::read(directory.join(name))
        .map_err(|_| "内置 Embedding 模型文件缺失，请重新安装完整资源。")?;
    verify_asset(&bytes, asset)?;
    Ok(bytes)
}

fn environment(_root: &Path) -> Result<ort::environment::Environment, String> {
    static ENVIRONMENT: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| {
            #[cfg(windows)]
            let builder = {
                if !cfg!(target_arch = "x86_64") {
                    return Err("内置 CPU 运行库当前仅随 Windows x64 安装包提供。".into());
                }
                let directory = _root.join("resource/runtime/windows-x86_64");
                let _ = read_verified(
                    &directory,
                    "onnxruntime.dll",
                    include_str!("../../resource/runtime/windows-x86_64/manifest.json"),
                )?;
                let path = directory
                    .join("onnxruntime.dll")
                    .canonicalize()
                    .map_err(|_| "内置 CPU 运行库路径不可用。")?;
                ort::init_from(path).map_err(|_| "无法加载内置 CPU 运行库，请重新安装完整软件。")?
            };
            #[cfg(not(windows))]
            let builder = ort::init();
            builder
                .with_telemetry(false)
                .build()
                .map_err(|_| "无法初始化内置 CPU 运行库。".into())
        })
        .clone()
}

fn verify_asset(bytes: &[u8], asset: &Asset) -> Result<(), String> {
    let digest: String = hmac_sha256::Hash::hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if bytes.len() != asset.size || digest != asset.sha256 {
        return Err("内置 Embedding 模型校验失败，请重新安装完整资源。".into());
    }
    Ok(())
}

impl LocalEmbedding {
    pub fn load(root: &Path, threads: usize) -> Result<Self, String> {
        let directory = root.join(MODEL_DIRECTORY);
        let model = UserDefinedEmbeddingModel::new(
            read_asset(&directory, "model.onnx")?,
            TokenizerFiles {
                tokenizer_file: read_asset(&directory, "tokenizer.json")?,
                config_file: read_asset(&directory, "config.json")?,
                special_tokens_map_file: read_asset(&directory, "special_tokens_map.json")?,
                tokenizer_config_file: read_asset(&directory, "tokenizer_config.json")?,
            },
        )
        .with_pooling(Pooling::Cls)
        .with_quantization(QuantizationMode::Dynamic);
        let options = InitOptionsUserDefined::new()
            .with_environment(environment(root)?)
            .with_max_length(512)
            .with_intra_threads(threads.clamp(1, 4))
            .with_auto_device(AutoDevicePolicy::PreferCPU)
            .with_session_config("session.intra_op.allow_spinning", "0")
            .with_session_config("session.inter_op.allow_spinning", "0");
        let model = TextEmbedding::try_new_from_user_defined(model, options)
            .map_err(|_| "内置 Embedding 模型初始化失败；本轮继续普通对话。")?;
        Ok(Self { model })
    }

    pub fn split(&self, text: &str, reserved_tokens: usize) -> Result<Vec<String>, String> {
        let mut tokenizer = self.model.tokenizer.clone();
        tokenizer
            .with_truncation(None)
            .map_err(|_| "内置模型分词配置失败。")?;
        tokenizer.with_padding(None);
        let encoded = tokenizer
            .encode(text, false)
            .map_err(|_| "内置模型分词失败。")?;
        split_at_offsets(
            text,
            encoded.get_offsets(),
            CONTENT_TOKENS.saturating_sub(reserved_tokens).max(1),
        )
    }

    pub fn embed_documents(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        let mut result = Vec::with_capacity(texts.len());
        for text in texts {
            let chunks = self.split(text, 0)?;
            if chunks.is_empty() {
                return Err("待生成向量的文本为空。".into());
            }
            let mut combined = vec![0.; 512];
            for chunk in chunks {
                let vector = self.embed_one(&chunk)?;
                for (sum, value) in combined.iter_mut().zip(vector) {
                    *sum += value;
                }
            }
            normalize(&mut combined)?;
            result.push(combined);
        }
        Ok(result)
    }

    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let parts = self.split(text, 32)?;
        let first = parts.first().ok_or("待检索的输入为空。")?;
        self.embed_one(&format!("{QUERY_PREFIX}{first}"))
    }

    fn embed_one(&mut self, text: &str) -> Result<Vec<f32>, String> {
        // Dynamic INT8 ranges depend on the batch. Always use a single sequence
        // so stored passages and later queries have consistent quantization.
        let mut vectors = self
            .model
            .embed([text], None)
            .map_err(|_| "内置 Embedding 计算失败；本轮继续普通对话。")?;
        let mut vector = vectors.pop().ok_or("内置 Embedding 未生成向量。")?;
        if vector.len() != 512 {
            return Err("内置 Embedding 向量维度无效。".into());
        }
        normalize(&mut vector)?;
        Ok(vector)
    }
}

fn normalize(vector: &mut [f32]) -> Result<(), String> {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err("内置 Embedding 向量无效。".into());
    }
    for value in vector {
        *value = (f64::from(*value) / norm) as f32;
    }
    Ok(())
}

/// Explicit offline installation check using fixed text only; never opens user records.
pub fn check(root: &Path) -> Result<serde_json::Value, String> {
    let started = std::time::Instant::now();
    let mut model = LocalEmbedding::load(root, 2)?;
    let loaded = started.elapsed();
    let documents = [
        "用户不喜欢喝咖啡，晚上会睡不着，喜欢温热的茶。",
        "今天修好了电脑的键盘。",
        "明天计划去公园散步。",
    ];
    let vectors = model.embed_documents(&documents)?;
    let query = model.embed_query("我晚上喝什么饮料比较合适？")?;
    let scores: Vec<f32> = vectors
        .iter()
        .map(|vector| vector.iter().zip(&query).map(|(a, b)| a * b).sum())
        .collect();
    let best = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(i, _)| i)
        .unwrap();
    if best != 0 || scores[0] < 0.5 || scores[1..].iter().any(|score| *score >= 0.5) {
        return Err("内置 Embedding 中文语义检索检查未通过。".into());
    }
    let long = "中文分词不会截断后半段🙂 mixed English 123。".repeat(100);
    let chunks = model.split(&long, 8)?;
    if chunks.concat() != long {
        return Err("内置 Embedding 长文本切分检查未通过。".into());
    }
    Ok(
        serde_json::json!({"model":"bge-small-zh-v1.5-int8","device":"cpu","threads":2,"dimensions":query.len(),"loadMs":loaded.as_millis(),"inferenceMs":started.elapsed().saturating_sub(loaded).as_millis(),"semanticCheck":true,"splitCheck":true,"scores":scores}),
    )
}

fn split_at_offsets(
    text: &str,
    offsets: &[(usize, usize)],
    tokens: usize,
) -> Result<Vec<String>, String> {
    if offsets.is_empty() {
        return Ok(vec![]);
    }
    let mut start = 0;
    let mut result = vec![];
    for chunk in offsets.chunks(tokens) {
        let end = chunk.last().unwrap().1;
        if end < start || !text.is_char_boundary(end) {
            return Err("内置模型分词边界无效。".into());
        }
        if end > start {
            result.push(text[start..end].to_owned());
        }
        start = end;
    }
    if start < text.len()
        && let Some(last) = result.last_mut()
    {
        last.push_str(&text[start..]);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_splitting_integrity_and_vector_normalization() {
        let text = "甲乙  a🙂尾 ";
        let offsets: Vec<_> = text
            .char_indices()
            .filter(|(_, ch)| !ch.is_whitespace())
            .map(|(i, ch)| (i, i + ch.len_utf8()))
            .collect();
        let chunks = split_at_offsets(text, &offsets, 2).unwrap();
        assert_eq!(chunks.concat(), text);
        assert!(chunks.len() > 1);
        assert!(split_at_offsets("甲", &[(0, 1)], 2).is_err());
        let bytes = b"fixture model";
        let digest: String = hmac_sha256::Hash::hash(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let asset = Asset {
            name: "fixture".into(),
            size: bytes.len(),
            sha256: digest,
        };
        assert!(verify_asset(bytes, &asset).is_ok());
        assert!(verify_asset(b"changed model", &asset).is_err());
        let mut vector = [3., 4.];
        normalize(&mut vector).unwrap();
        assert!((vector[0] - 0.6).abs() < 0.00001);
        assert!(normalize(&mut [0., 0.]).is_err());
        assert!(normalize(&mut [f32::NAN]).is_err());
    }
}
