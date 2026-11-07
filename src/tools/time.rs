pub fn local(timestamp: i64) -> Option<libc::tm> {
    let native = timestamp as libc::time_t;
    if native as i128 != i128::from(timestamp) {
        return None;
    }
    let mut date = std::mem::MaybeUninit::<libc::tm>::uninit();
    #[cfg(unix)]
    let success = !unsafe { libc::localtime_r(&native, date.as_mut_ptr()) }.is_null();
    #[cfg(windows)]
    let success = unsafe { libc::localtime_s(date.as_mut_ptr(), &native) } == 0;
    if success {
        Some(unsafe { date.assume_init() })
    } else {
        None
    }
}

pub fn timestamp(date: &mut libc::tm) -> i64 {
    #[cfg(unix)]
    unsafe {
        libc::mktime(date) as i64
    }
    #[cfg(windows)]
    unsafe {
        unsafe extern "C" {
            #[link_name = "_mktime64"]
            fn windows_mktime(date: *mut libc::tm) -> i64;
        }
        windows_mktime(date)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn local_calendar_round_trips_and_rejects_extreme_timestamps() {
        for timestamp in [1_700_000_000, 1_800_000_000, 1_900_000_000] {
            let mut date = super::local(timestamp).unwrap();
            assert_eq!(super::timestamp(&mut date), timestamp);
        }
        assert!(super::local(i64::MAX).is_none());
    }
}
