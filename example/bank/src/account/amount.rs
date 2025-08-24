use serde::{Deserialize, Serialize};
use std::ops::{AddAssign, Mul, SubAssign};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Amount {
    pub value: f64,
    pub currency: String,
}

impl Amount {
    pub fn new(value: f64, currency: String) -> Self {
        Self { value, currency }
    }

    pub fn abs(&self) -> Self {
        Self {
            value: self.value.abs(),
            currency: self.currency.clone(),
        }
    }
}

impl Mul<f64> for Amount {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            value: self.value * rhs,
            currency: self.currency.clone(),
        }
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self {
            value: 0f64,
            currency: "EUR".to_string(),
        }
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.value -= rhs.value;
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.value += rhs.value;
    }
}

impl From<f64> for Amount {
    fn from(value: f64) -> Self {
        Self::new(value, "EUR".to_string())
    }
}
