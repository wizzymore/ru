const UNITS: [&str; 9] = ["B", "K", "M", "G", "T", "P", "E", "Z", "Y"];

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
/// Holds the standard to use when displaying the size.
pub enum Kilo {
    /// The decimal scale and units. SI standard.
    #[default]
    Decimal,
    /// The binary scale and units.
    #[cfg_attr(not(target_os = "linux"), expect(dead_code))]
    Binary,
}

impl AsRef<Kilo> for Kilo {
    fn as_ref(&self) -> &Kilo {
        self
    }
}

impl Kilo {
    pub(crate) fn value(&self) -> f64 {
        match self {
            Kilo::Decimal => 1000.0,
            Kilo::Binary => 1024.0,
        }
    }
}

pub trait ToF64 {
    fn to_f64(&self) -> f64;
}

macro_rules! impl_to_f64 {
  (for $($t:ty)*) => ($(
      impl ToF64 for $t {
          fn to_f64(&self) -> f64 {
              *self as f64
          }
      }
  )*)
}

impl_to_f64!(for usize u8 u16 u32 u64 isize i8 i16 i32 i64 f32 f64);

pub struct ISizeFormatter<T: ToF64, O: AsRef<Kilo>> {
    value: T,
    options: O,
}

impl<V: ToF64, O: AsRef<Kilo>> ISizeFormatter<V, O> {
    pub fn new(value: V, options: O) -> Self {
        ISizeFormatter { value, options }
    }
}

impl<T: ToF64, O: AsRef<Kilo>> core::fmt::Display for ISizeFormatter<T, O> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let opts = self.options.as_ref();
        let divider = opts.value();

        let mut size: f64 = self.value.to_f64();
        let mut scale_idx = 0;

        while size.abs() >= divider {
            size /= divider;
            scale_idx += 1;
        }

        let places: usize = match size {
            10.0.. => 0,
            _ => 1,
        };

        if places == 0 {
            return write!(f, "{:.0}{}", size, UNITS[scale_idx]);
        }

        if cfg!(unix) {
            unsafe {
                let conv = libc::localeconv();
                let decimal = std::ffi::CStr::from_ptr((*conv).decimal_point)
                    .to_str()
                    .unwrap_or(".");

                return write!(
                    f,
                    "{:.0}{}{:.0}{}",
                    size.trunc(),
                    decimal,
                    size.fract() * (10 * places) as f64,
                    UNITS[scale_idx]
                );
            }
        }

        write!(f, "{:.*}{}", places, size, UNITS[scale_idx])
    }
}

pub fn format_size(input: impl ToF64, options: impl AsRef<Kilo>) -> String {
    format!("{}", ISizeFormatter::new(input, options))
}
