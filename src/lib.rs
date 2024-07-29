mod space;
mod sys;

pub use space::*;

use dlopen2::wrapper::Container;
use flagset::FlagSet;
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::env;
use std::ffi::c_char;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use std::vec;
use sys::ClientState;
use sys::MndProperty;
use sys::MndResult;
use sys::MndRootPtr;
use sys::MonadoApi;

fn crate_api_version() -> VersionReq {
	VersionReq::parse("^1.3.0").unwrap()
}
fn get_api_version(api: &Container<MonadoApi>) -> Version {
	let mut major = 0;
	let mut minor = 0;
	let mut patch = 0;
	unsafe { api.mnd_api_get_version(&mut major, &mut minor, &mut patch) };

	Version::new(major as u64, minor as u64, patch as u64)
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeJSON {
	runtime: RuntimeInfo,
}
#[derive(Debug, Clone, Deserialize)]
struct RuntimeInfo {
	#[serde(rename = "library_path")]
	_library_path: PathBuf,
	#[serde(rename = "MND_libmonado_path")]
	libmonado_path: Option<PathBuf>,
}

pub struct Monado {
	api: Container<MonadoApi>,
	root: MndRootPtr,
}
impl Monado {
	pub fn auto_connect() -> Result<Self, String> {
		if let Ok(libmonado_path) = env::var("LIBMONADO_PATH") {
			match fs::metadata(&libmonado_path) {
				Ok(metadata) if metadata.is_file() => {
					return Self::create(libmonado_path).map_err(|e| format!("{e:?}"))
				}
				_ => return Err("LIBMONADO_PATH does not point to a valid file".into()),
			}
		}

		let active_runtime = xdg::BaseDirectories::new()
			.map_err(|e| format!("{e:?}"))?
			.find_config_files("openxr/1/active_runtime.json")
			.filter_map(|p| p.canonicalize().ok())
			.filter_map(|p| Some((std::fs::read_to_string(&p).ok()?, p)))
			.filter_map(|(j, p)| Some((serde_json::from_str::<RuntimeJSON>(&j).ok()?, p)))
			.next();

		let Some((runtime_json, mut runtime_path)) = active_runtime else {
			return Err("Couldn't find the actively running runtime".to_string());
		};
		runtime_path.pop();
		let Some(libmonado_path) = runtime_json.runtime.libmonado_path else {
			return Err("Couldn't find libmonado path in active runtime json".to_string());
		};

		let path = runtime_path.join(libmonado_path);
		Self::create(path).map_err(|e| format!("{e:?}"))
	}
	pub fn create<S: AsRef<OsStr>>(libmonado_so: S) -> Result<Self, MndResult> {
		let api = unsafe { Container::<MonadoApi>::load(libmonado_so) }
			.map_err(|_| MndResult::ErrorConnectingFailed)?;
		if !crate_api_version().matches(&get_api_version(&api)) {
			return Err(MndResult::ErrorInvalidVersion);
		}
		let mut root = std::ptr::null_mut();
		unsafe {
			api.mnd_root_create(&mut root).to_result()?;
		}
		Ok(Monado { api, root })
	}

	pub fn get_api_version(&self) -> Version {
		get_api_version(&self.api)
	}
	pub fn recenter_local_spaces(&self) -> Result<(), MndResult> {
		unsafe {
			self.api
				.mnd_root_recenter_local_spaces(self.root)
				.to_result()
		}
	}

	pub fn clients(&self) -> Result<impl IntoIterator<Item = Client<'_>>, MndResult> {
		unsafe {
			self.api
				.mnd_root_update_client_list(self.root)
				.to_result()?
		};
		let mut count = 0;
		unsafe {
			self.api
				.mnd_root_get_number_clients(self.root, &mut count)
				.to_result()?
		};
		let mut clients: Vec<Option<Client>> = vec::from_elem(None, count as usize);
		for (index, client) in clients.iter_mut().enumerate() {
			let mut id = 0;
			unsafe {
				self.api
					.mnd_root_get_client_id_at_index(self.root, index as u32, &mut id)
					.to_result()?
			};
			client.replace(Client { monado: self, id });
		}
		Ok(clients.into_iter().flatten())
	}

	// Get device id from role name
	//
	// @param root Opaque libmonado state
	// @param role_name Name of the role
	// @param out_index Pointer to populate with device id
	pub fn device_from_role<'m>(&'m self, role_name: &str) -> Result<Device<'m>, MndResult> {
		let c_name = CString::new(role_name).unwrap();
		let mut index = -1;

		unsafe {
			self.api
				.mnd_root_get_device_from_role(self.root, c_name.as_ptr(), &mut index)
				.to_result()?
		};
		let mut id = 0;
		let mut c_name: *const c_char = std::ptr::null_mut();
		unsafe {
			self.api
				.mnd_root_get_device_info(self.root, index as u32, &mut id, &mut c_name)
				.to_result()?
		};
		let name = unsafe {
			CStr::from_ptr(c_name)
				.to_str()
				.map_err(|_| MndResult::ErrorInvalidValue)?
				.to_owned()
		};

		Ok(Device {
			monado: self,
			index: index as u32,
			id,
			name,
		})
	}

	pub fn devices(&self) -> Result<impl IntoIterator<Item = Device<'_>>, MndResult> {
		let mut count = 0;
		unsafe {
			self.api
				.mnd_root_get_device_count(self.root, &mut count)
				.to_result()?
		};
		let mut devices: Vec<Option<Device>> = vec::from_elem(None, count as usize);
		for (index, device) in devices.iter_mut().enumerate() {
			let index = index as u32;
			let mut id = 0;
			let mut c_name: *const c_char = std::ptr::null_mut();
			unsafe {
				self.api
					.mnd_root_get_device_info(self.root, index, &mut id, &mut c_name)
					.to_result()?
			};
			let name = unsafe {
				CStr::from_ptr(c_name)
					.to_str()
					.map_err(|_| MndResult::ErrorInvalidValue)?
					.to_owned()
			};
			device.replace(Device {
				monado: self,
				index,
				id,
				name,
			});
		}
		Ok(devices.into_iter().flatten())
	}
}
impl Drop for Monado {
	fn drop(&mut self) {
		unsafe { self.api.mnd_root_destroy(&mut self.root) }
	}
}

#[derive(Clone)]
pub struct Client<'m> {
	monado: &'m Monado,
	id: u32,
}
impl Client<'_> {
	pub fn name(&mut self) -> Result<String, MndResult> {
		let mut string = std::ptr::null();
		unsafe {
			self.monado
				.api
				.mnd_root_get_client_name(self.monado.root, self.id, &mut string)
				.to_result()?
		};
		let c_string = unsafe { CStr::from_ptr(string) };
		c_string
			.to_str()
			.map_err(|_| MndResult::ErrorInvalidValue)
			.map(ToString::to_string)
	}
	pub fn state(&mut self) -> Result<FlagSet<ClientState>, MndResult> {
		let mut state = 0;
		unsafe {
			self.monado
				.api
				.mnd_root_get_client_state(self.monado.root, self.id, &mut state)
				.to_result()?
		};
		Ok(unsafe { FlagSet::new_unchecked(state) })
	}
	pub fn set_primary(&mut self) -> Result<(), MndResult> {
		unsafe {
			self.monado
				.api
				.mnd_root_set_client_primary(self.monado.root, self.id)
				.to_result()
		}
	}
	pub fn set_focused(&mut self) -> Result<(), MndResult> {
		unsafe {
			self.monado
				.api
				.mnd_root_set_client_focused(self.monado.root, self.id)
				.to_result()
		}
	}
	pub fn set_io_active(&mut self, active: bool) -> Result<(), MndResult> {
		let state = self.state()?;
		if state.contains(ClientState::ClientIoActive) != active {
			unsafe {
				self.monado
					.api
					.mnd_root_toggle_client_io_active(self.monado.root, self.id)
					.to_result()?;
			}
		}
		Ok(())
	}
}

#[derive(Clone)]
pub struct Device<'m> {
	monado: &'m Monado,
	pub index: u32,
	pub id: u32,
	pub name: String,
}
impl Device<'_> {
	pub fn serial(&self) -> Result<String, MndResult> {
		self.get_info_string(MndProperty::PropertySerialString)
	}
	pub fn get_info_bool(&self, property: MndProperty) -> Result<bool, MndResult> {
		let mut value: bool = Default::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_device_info_bool(self.monado.root, self.index, property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	pub fn get_info_u32(&self, property: MndProperty) -> Result<u32, MndResult> {
		let mut value: u32 = Default::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_device_info_u32(self.monado.root, self.index, property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	pub fn get_info_i32(&self, property: MndProperty) -> Result<i32, MndResult> {
		let mut value: i32 = Default::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_device_info_i32(self.monado.root, self.index, property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	pub fn get_info_f32(&self, property: MndProperty) -> Result<f32, MndResult> {
		let mut value: f32 = Default::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_device_info_float(self.monado.root, self.index, property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	pub fn get_info_string(&self, property: MndProperty) -> Result<String, MndResult> {
		let value = CString::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_device_info_string(
					self.monado.root,
					self.index,
					property,
					value.as_ptr(),
				)
				.to_result()?
		}
		Ok(value.to_string_lossy().to_string())
	}
}
impl Debug for Device<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Device")
			.field("id", &self.id)
			.field("name", &self.name)
			.finish()
	}
}

#[test]
fn test_dump_info() {
	let monado = Monado::auto_connect().unwrap();
	dbg!(monado.get_api_version());
	println!();

	for mut client in monado.clients().unwrap() {
		dbg!(client.name().unwrap(), client.state().unwrap());
		println!();
	}
	for device in monado.devices().unwrap() {
		let _ = dbg!(device.id, &device.name, device.serial());
		println!();
	}
	for tracking_origin in monado.tracking_origins().unwrap() {
		dbg!(
			tracking_origin.id,
			&tracking_origin.name,
			tracking_origin.get_offset().unwrap()
		);
		println!();
	}
}
