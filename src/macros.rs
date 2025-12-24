#[macro_export]
macro_rules! ext_fn {
    ($($p:vis fn ($f:ty) $a:ident($($arg:tt)*) $(-> $ret:ty)? $sts:block)*) => {
        $(
            #[allow(non_camel_case_types)]
            $p trait $a {
                fn $a($($arg)*) $(-> $ret)?;
            }
            impl $a for $f {
                fn $a($($arg)*) $(-> $ret)? $sts
            }
        )*
    };
}
