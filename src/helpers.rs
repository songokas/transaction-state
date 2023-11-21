#[macro_export]
macro_rules! curry {
    ($f:expr,$s:expr) => {{
        let o = $s;
        |p| async move { $f(&o, p).await }
    }};
}

#[macro_export]
macro_rules! curry2 {
    ($f:expr,$s:expr) => {{
        let o = $s;
        |(a, b)| async move { $f(&o, a, b).await }
    }};
}

#[macro_export]
macro_rules! curry3 {
    ($f:expr,$s:expr) => {{
        let o = $s;
        |(a, b, c)| async move { $f(&o, a, b, c).await }
    }};
}

#[macro_export]
macro_rules! curry4 {
    ($f:expr,$s:expr) => {{
        let o = $s;
        |(a, b, c, d)| async move { $f(&o, a, b, c, d).await }
    }};
}


#[cfg(test)]
mod tests {
    async fn test1(a: &str, b: String) -> String {
        format!("{a}_{b}")
    }
    async fn test2(a: &str, b: String, c: String) -> String {
        format!("{a}_{b}{c}")
    }
    async fn test3(a: &str, b: String, c: String, d: String) -> String {
        format!("{a}_{b}{c}{d}")
    }
    async fn test4(a: &str, b: String, c: String, d: String, e: String) -> String {
        format!("{a}_{b}{c}{d}{e}")
    }

    #[tokio::test]
    async fn test_all() {
        let f = curry!(test1, "test1".to_string());
        assert_eq!("test1_1", f("1".to_string()).await);
        let f = curry2!(test2, "test2".to_string());
        assert_eq!("test2_12", f(("1".to_string(), "2".to_string())).await);
        let f = curry3!(test3, "test3".to_string());
        assert_eq!(
            "test3_123",
            f(("1".to_string(), "2".to_string(), "3".to_string())).await
        );
        let f = curry4!(test4, "test4".to_string());
        assert_eq!(
            "test4_1234",
            f((
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string()
            ))
            .await
        );
    }
}
