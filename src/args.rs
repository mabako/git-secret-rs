use std::ffi::OsString;

use crate::AppResult;

pub(crate) struct Args {
    values: std::vec::IntoIter<OsString>,
}

impl Args {
    pub(crate) fn new(values: Vec<OsString>) -> Self {
        Self {
            values: values.into_iter(),
        }
    }

    pub(crate) fn next_string(&mut self) -> Option<String> {
        self.values
            .next()
            .map(|value| value.to_string_lossy().into())
    }

    pub(crate) fn rest_strings(self) -> AppResult<Vec<String>> {
        self.values
            .map(|value| {
                value
                    .into_string()
                    .map_err(|_| "argument is not valid UTF-8".to_string())
            })
            .collect()
    }
}
