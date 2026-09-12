use std::path::Path;

use oci_spec::runtime::Spec;

use crate::error::{KuroError, Result};

pub fn load_spec(bundle: &Path) -> Result<Spec> {
    let spec_path = bundle.join("config.json");
    if !spec_path.is_file() {
        return Err(KuroError::InvalidBundle {
            path: spec_path,
            reason: "Missing config.json in bundle directory".to_string(),
        });
    }

    let spec = Spec::load(spec_path)?;

    Ok(spec)
}
