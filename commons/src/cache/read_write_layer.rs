use itertools::Itertools;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::LayerData;
use libcnb::layer::LayerResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::layer::{GetLayerData, SetLayerData};

#[derive(Debug, Clone)]
pub enum MetadataDiff<M> {
    Same(M),
    Different {
        old: M,
        now: M,
    },
    CannotDeserialize {
        old: Option<toml::value::Table>,
        now: M,
    },
    NoCache(M),
}

#[derive(Debug, Clone)]
pub struct TomlDelta {
    pub key: String,
    pub old: Option<toml::Value>,
    pub now: Option<toml::Value>,
}

/// Returns the delta between the two objects
pub fn toml_delta<M: Serialize>(old: &M, now: &M) -> Vec<TomlDelta> {
    let old = toml::to_string(&old)
        .ok()
        .and_then(|string| string.parse::<toml::Table>().ok())
        .unwrap_or_default();

    let now = toml::to_string(&now)
        .ok()
        .and_then(|string| string.parse::<toml::Table>().ok())
        .unwrap_or_default();

    let mut diff = Vec::new();
    for key in old.keys().chain(now.keys()).unique() {
        match (old.get(key).cloned(), now.get(key).cloned()) {
            (old_value, now_value) if old_value != now_value => diff.push(TomlDelta {
                key: key.clone(),
                old: old_value.clone(),
                now: now_value.clone(),
            }),
            _ => {}
        }
    }

    diff
}

pub fn metadata_diff<M>(raw_metadata: &Option<toml::Table>, metadata: M) -> MetadataDiff<M>
where
    M: Serialize + DeserializeOwned + Eq + PartialEq + Clone,
{
    let cache_data = raw_metadata.clone();
    if let Some(cache) = cache_data.clone() {
        let old: Result<M, toml::de::Error> = cache.try_into();
        match &old {
            Ok(old) => {
                if old == &metadata {
                    MetadataDiff::Same(metadata)
                } else {
                    MetadataDiff::Different {
                        old: old.clone(),
                        now: metadata.clone(),
                    }
                }
            }
            Err(_) => MetadataDiff::CannotDeserialize {
                old: cache_data,
                now: metadata,
            },
        }
    } else {
        MetadataDiff::NoCache(metadata)
    }
}

pub struct CachedLayer<M> {
    pub name: LayerName,
    pub build: bool,
    pub launch: bool,
    pub metadata: M,
}

impl<M> CachedLayer<M>
where
    M: Serialize + DeserializeOwned + Eq + PartialEq + Clone,
{
    /// # Errors
    ///
    /// TODO
    pub fn read<B: libcnb::Buildpack>(
        &self,
        context: &BuildContext<B>,
    ) -> Result<CachedLayerData<B, M>, libcnb::Error<B::Error>> {
        let (read_name, write_name) = (clone_name(&self.name), clone_name(&self.name));

        let data = context //
            .handle_layer(
                read_name,
                GetLayerData::new(LayerTypes {
                    cache: true,
                    launch: self.launch,
                    build: self.build,
                }),
            )?;

        let name = write_name;
        let path = data.path.clone();
        let metadata_diff = metadata_diff(&data.content_metadata.metadata, self.metadata.clone());
        let buildpack = PhantomData::<B>;
        let layer_types = LayerTypes {
            cache: true,
            build: self.build,
            launch: self.launch,
        };

        Ok(CachedLayerData {
            name,
            path,
            layer_types,
            metadata_diff,
            buildpack,
        })
    }
}

fn clone_name(name: &LayerName) -> LayerName {
    name.as_str()
        .parse::<LayerName>()
        .expect("Parsing a layer name from a layer name should be infailable")
}

pub struct CachedLayerData<B, M> {
    pub name: LayerName,
    pub path: PathBuf,
    pub layer_types: LayerTypes,
    pub metadata_diff: MetadataDiff<M>,

    buildpack: PhantomData<B>,
}

impl<B, M> CachedLayerData<B, M> {
    /// # Errors
    ///
    /// TODO
    pub fn clear_path_contents(&self) -> Result<(), std::io::Error> {
        // Ideally would return licnb::Error::HandleLayerError but the internal type not exposed
        fs_err::remove_dir_all(&self.path)?;
        fs_err::create_dir_all(&self.path)
    }
}

impl<B, M> CachedLayerData<B, M>
where
    M: Serialize + DeserializeOwned + Eq + PartialEq + Clone,
    B: libcnb::Buildpack,
{
    /// # Errors
    ///
    /// TODO
    pub fn write(
        &self,
        context: &BuildContext<B>,
        layer_result: LayerResult<M>,
    ) -> Result<LayerData<M>, libcnb::Error<B::Error>> {
        context.handle_layer(
            clone_name(&self.name),
            SetLayerData::new(
                LayerTypes {
                    cache: self.layer_types.cache,
                    build: self.layer_types.build,
                    launch: self.layer_types.launch,
                },
                layer_result,
            ),
        )
    }

    pub fn diff(&self) -> MetadataDiff<M> {
        self.metadata_diff.clone()
    }
}
