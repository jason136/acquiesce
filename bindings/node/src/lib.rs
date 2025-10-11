// use std::path::Path;

// use acquiesce::{configs::kimik2::kimi_k2, AcquiesceInit};
// use napi::bindgen_prelude::*;
// use napi_derive::napi;

// struct HeavyTask {
//     input: Vec<u8>,
// }

// #[napi]
// impl Task for HeavyTask {
//     type Output = Vec<u8>;
//     type JsValue = Vec<u8>;

//     fn compute(&mut self) -> Result<Self::Output> {
//         let mut transformed = self.input.clone();
//         for byte in &mut transformed {
//             *byte = byte.wrapping_mul(31).rotate_left(3);
//         }
//         Ok(transformed)
//     }

//     fn resolve(&mut self, _env: Env, out: Self::Output) -> Result<Self::JsValue> {
//         Ok(out)
//     }
// }

// #[napi]
// pub fn heavy_transform(input: Buffer) -> AsyncTask<HeavyTask> {
//     AsyncTask::new(HeavyTask {
//         input: input.to_vec(),
//     })
// }

// #[napi]
// pub struct AcquiesceHandle {
//     inner: AcquiesceInit,
// }

// #[napi]
// impl AcquiesceHandle {
//     #[napi(constructor)]
//     pub fn from_repo_with_fallback(path: String, fallback_name: Option<String>) -> Result<Self> {
//         let inner = if let Some(fallback_name) = fallback_name {
//             let fallback = match fallback_name.as_str() {
//                 "kimi" => kimi_k2(),
//                 _ => return Err(Error::new(Status::InvalidArg, "Invalid fallback name")),
//             };

//             AcquiesceInit::from_repo_with_fallback(Path::new(&path), fallback)
//         } else {
//             AcquiesceInit::from_repo(Path::new(&path))
//         }
//         .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

//         Ok(Self { inner })
//     }

//     // // Demonstrate using the async Task pattern bound to this instance
//     // #[napi]
//     // pub fn transform_async(&self) -> AsyncTask<HeavyTask> {
//     //     AsyncTask::new(HeavyTask { input: self.inner })
//     // }
// }
