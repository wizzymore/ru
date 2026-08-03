const UNITS: [&str; 9] = ["B", "K", "M", "G", "T", "P", "E", "Z", "Y"];

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
/// Holds the standard to use when displaying the size.
pub enum Kilo {
    /// The decimal scale and units. SI standard.
    #[default]
    Decimal,
    /// The binary scale and units.
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

pub struct ISizeFormatter<T: Into<f64> + Copy, O: AsRef<Kilo>> {
    value: T,
    options: O,
}

impl<V: Into<f64> + Copy, O: AsRef<Kilo>> ISizeFormatter<V, O> {
    pub fn new(value: V, options: O) -> Self {
        ISizeFormatter { value, options }
    }
}

impl<T: Into<f64> + Copy, O: AsRef<Kilo>> core::fmt::Display for ISizeFormatter<T, O> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let opts = self.options.as_ref();
        let divider = opts.value();

        let mut size: f64 = self.value.into();
        let mut scale_idx = 0;

        while size.abs() >= divider {
            size /= divider;
            scale_idx += 1;
        }

        let mut scale = UNITS[scale_idx];

        // Remove "s" from the scale if the size is 1.x
        if size.fract() == 1.0 {
            scale = &scale[0..scale.len() - 1];
        }

        write!(f, "{:.2}{}", size, scale)
    }
}

pub fn format_size_i(input: impl Into<f64> + Copy, options: impl AsRef<Kilo>) -> String {
    format!("{}", ISizeFormatter::new(input, options))
}

pub fn format_size(input: impl Into<f64> + Copy, options: impl AsRef<Kilo>) -> String {
    format_size_i(input, &options)
}
