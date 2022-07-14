#[macro_export]
macro_rules! utf16z {
    ($str: expr) => {
        $str.encode_utf16().chain([0]).collect::<Vec<_>>()
    };
}
