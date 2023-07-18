use crate::MetricsAgentBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::Buildpack;
use std::path::Path;

use super::download_agentmon;

pub(crate) struct SpawnAgentmonExecd;

impl Layer for SpawnAgentmonExecd {
    type Buildpack = MetricsAgentBuildpack;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            launch: true,
            build: false,
            cache: false,
        }
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let path = layer_path.join("lol");
        // Intentionally leak background process
        let script = r#"#!/usr/bin/env bash

            #!/usr/bin/env bash

            echo "spawning agentmon" &

            # Intentional leak of process
            # https://superuser.com/questions/448445/run-bash-script-in-background-and-exit-terminal
            while true; do echo "pretend agentmon"; sleep 2; done &
        "#;

        fs_err::write(&path, script).unwrap();
        download_agentmon::chmod_plus_x(&path).unwrap();

        LayerResultBuilder::new(GenericMetadata::default())
            .exec_d_program("spawn agentmon", path)
            .build()
    }
}
