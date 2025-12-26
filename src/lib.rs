mod space;
mod sys;

pub use semver::Version;
pub use space::*;
pub use sys::ClientState;
pub use sys::MndProperty;
pub use sys::MndResult;

use dlopen2::wrapper::Container;
use flagset::FlagSet;
use semver::VersionReq;
use serde::Deserialize;
use std::env;
use std::ffi::*;
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::vec;
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

#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
	pub present: bool,
	pub charging: bool,
	pub charge: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum DeviceRole {
	Head,
	Eyes,
	Left,
	Right,
	Gamepad,
	HandTrackingLeft,
	HandTrackingRight,
}

impl From<DeviceRole> for &'static str {
	fn from(value: DeviceRole) -> Self {
		match value {
			DeviceRole::Head => "head",
			DeviceRole::Eyes => "eyes",
			DeviceRole::Left => "left",
			DeviceRole::Right => "right",
			DeviceRole::Gamepad => "gamepad",
			DeviceRole::HandTrackingLeft => "hand-tracking-left",
			DeviceRole::HandTrackingRight => "hand-tracking-right",
		}
	}
}

#[cfg(unix)]
fn find_system_library(lib: &str) -> Option<PathBuf> {
	let lib = CString::new(lib).expect("library name isn't a valid C string");

	let handle = unsafe { libc::dlopen(lib.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
	if handle.is_null() {
		return None;
	}

	struct Handle(*mut c_void);
	impl Drop for Handle {
		fn drop(&mut self) {
			unsafe { libc::dlclose(self.0) };
		}
	}
	let handle = Handle(handle);

	#[cfg(target_pointer_width = "32")]
	use libc::Elf32_Addr as ElfAddr;

	#[cfg(target_pointer_width = "64")]
	use libc::Elf64_Addr as ElfAddr;

	#[repr(C)]
	struct LinkMap {
		addr: ElfAddr,
		name: *mut c_char,
		ld: *mut (),
		next: *mut LinkMap,
		prev: *mut LinkMap,
	}

	let mut link_map = std::mem::MaybeUninit::<*mut LinkMap>::zeroed();
	let r = unsafe {
		libc::dlinfo(
			handle.0,
			libc::RTLD_DI_LINKMAP,
			link_map.as_mut_ptr() as *mut _,
		)
	};

	if r != 0 {
		return None;
	}

	let link_map = unsafe { &*link_map.assume_init() };
	let path = unsafe { CStr::from_ptr(link_map.name) };

	path.to_str().map(PathBuf::from).ok()
}

#[cfg(not(unix))]
fn find_system_library(lib: &str) -> Option<PathBuf> {
	None
}

fn resolve_runtime_library(lib: &Path, runtime_json_path: &Path) -> Result<PathBuf, String> {
	// Resolve relative to the real file, not the symlink.
	let mut runtime_path = std::fs::canonicalize(runtime_json_path)
		.map_err(|err| format!("Failed to canonicalize runtime json path: {}", err.kind()))?;
	runtime_path.pop();

	let path = runtime_path.join(lib);

	// Relative paths are always resolved relative to the location of active_runtime.json.
	if lib.components().count() > 1 {
		return Ok(path);
	}

	// Attempt to resolve bare filenames through the system's library search path.
	let lib = lib
		.to_str()
		.ok_or_else(|| "Library name contains invalid Unicode characters".to_string())?;

	if let Some(system_path) = find_system_library(lib) {
		return Ok(system_path);
	}

	// Fall back to the relative-path resolution mechanism if we can't locate the library in the system search path.
	Ok(path)
}

#[derive(Clone)]
struct DeviceData {
	index: u32,
	/// non-unique numeric representation of device name, see: xrt_device_name
	name_id: u32,
	name: String,
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

		let override_runtime = std::env::var_os("XR_RUNTIME_JSON").map(PathBuf::from);
		let possible_config_files = xdg::BaseDirectories::new()
			.ok()
			.into_iter()
			.flat_map(|b| b.find_config_files("openxr/1/active_runtime.json"))
			.rev();
		let override_runtime = override_runtime
			.into_iter()
			.chain(possible_config_files)
			.find_map(|p| {
				Some((
					serde_json::from_str::<RuntimeJSON>(&std::fs::read_to_string(&p).ok()?).ok()?,
					p,
				))
			});

		let Some((runtime_json, runtime_json_path)) = override_runtime else {
			return Err("Couldn't find the active runtime json".to_string());
		};

		let Some(libmonado_path) = runtime_json.runtime.libmonado_path else {
			return Err("Couldn't find libmonado path in active runtime json".to_string());
		};

		let path = resolve_runtime_library(&libmonado_path, &runtime_json_path)?;

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

	fn client_ids(&self) -> Result<impl IntoIterator<Item = u32>, MndResult> {
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
		let mut clients: Vec<Option<u32>> = vec::from_elem(None, count as usize);
		for (index, client) in clients.iter_mut().enumerate() {
			let mut id = 0;
			unsafe {
				self.api
					.mnd_root_get_client_id_at_index(self.root, index as u32, &mut id)
					.to_result()?
			};
			client.replace(id);
		}
		Ok(clients.into_iter().flatten())
	}

	pub fn clients(&self) -> Result<impl IntoIterator<Item = Client>, MndResult> {
		self.client_ids().map(|res| {
			res.into_iter().map(|id| Client {
				id,
				monado: self,
			})
		})
	}

    #[cfg(feature = "arc")]
	pub fn clients_arc(this: &std::sync::Arc<Self>) -> Result<Vec<ClientArc>, MndResult> {
		this.client_ids().map(|res| {
			res.into_iter().map(|id| ClientArc {
				id,
				monado: this.clone(),
			}).collect()
		})
	}

    #[cfg(feature = "rc")]
	pub fn clients_rc(this: &std::rc::Rc<Self>) -> Result<Vec<ClientRc>, MndResult> {
		this.client_ids().map(|res| {
			res.into_iter().map(|id| ClientRc {
				id,
				monado: this.clone(),
			}).collect()
		})
	}

	fn device_index_from_role_str(&self, role_name: &str) -> Result<u32, MndResult> {
		let c_name = CString::new(role_name).unwrap();
		let mut index = -1;

		unsafe {
			self.api
				.mnd_root_get_device_from_role(self.root, c_name.as_ptr(), &mut index)
				.to_result()?
		};
		if index == -1 {
			return Err(MndResult::ErrorInvalidValue);
		}
		Ok(index as u32)
	}

	// Get device id from role name
	//
	// @param root Opaque libmonado state
	// @param role_name Name of the role
	// @param out_index Pointer to populate with device id
	fn device_from_role_str<'m>(&'m self, role_name: &str) -> Result<Device<'m>, MndResult> {
		let index = self.device_index_from_role_str(role_name)?;
		let mut c_name: *const c_char = std::ptr::null_mut();
		let mut name_id = 0;
		unsafe {
			self.api
				.mnd_root_get_device_info(self.root, index, &mut name_id, &mut c_name)
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
			index,
			name_id,
			name,
		})
	}

	pub fn device_index_from_role(&self, role: DeviceRole) -> Result<u32, MndResult> {
		self.device_index_from_role_str(role.into())
	}

	pub fn device_from_role(&self, role: DeviceRole) -> Result<Device<'_>, MndResult> {
		self.device_from_role_str(role.into())
	}

	fn devices_data(&self) -> Result<impl IntoIterator<Item = DeviceData>, MndResult> {
		let mut count = 0;
		unsafe {
			self.api
				.mnd_root_get_device_count(self.root, &mut count)
				.to_result()?
		};
		let mut devices: Vec<Option<DeviceData>> = vec::from_elem(None, count as usize);
		for (index, device) in devices.iter_mut().enumerate() {
			let index = index as u32;
			let mut name_id = 0;
			let mut c_name: *const c_char = std::ptr::null_mut();
			unsafe {
				self.api
					.mnd_root_get_device_info(self.root, index, &mut name_id, &mut c_name)
					.to_result()?
			};
			let name = unsafe {
				CStr::from_ptr(c_name)
					.to_str()
					.map_err(|_| MndResult::ErrorInvalidValue)?
					.to_owned()
			};
			device.replace(DeviceData {
				index,
				name_id,
				name,
			});
		}
		Ok(devices.into_iter().flatten())
	}

	pub fn devices(&self) -> Result<impl IntoIterator<Item = Device<'_>>, MndResult> {
		self.devices_data().map(|res| {
			res.into_iter().map(|d| Device {
				index: d.index,
				name_id: d.name_id,
				name: d.name,
				monado: self,
			})
		})
	}

    #[cfg(feature = "arc")]
	pub fn devices_arc(this: &std::sync::Arc<Self>) -> Result<Vec<DeviceArc>, MndResult> {
		let data = this.devices_data();
		data.map(|res| {
			res.into_iter()
				.map(|d| DeviceArc {
					index: d.index,
					name_id: d.name_id,
					name: d.name,
					monado: this.clone(),
				})
				.collect()
		})
	}

    #[cfg(feature = "rc")]
	pub fn devices_rc(this: &std::rc::Rc<Self>) -> Result<Vec<DeviceRc>, MndResult> {
		let data = this.devices_data();
		data.map(|res| {
			res.into_iter()
				.map(|d| DeviceRc {
					index: d.index,
					name_id: d.name_id,
					name: d.name,
					monado: this.clone(),
				})
				.collect()
		})
	}
}
impl Drop for Monado {
	fn drop(&mut self) {
		unsafe { self.api.mnd_root_destroy(&mut self.root) }
	}
}

pub trait MonadoRef {
	fn monado(&self) -> &Monado;
}

pub trait ClientLogic: MonadoRef {
	fn id(&self) -> u32;

	fn name(&mut self) -> Result<String, MndResult> {
		let mut string = std::ptr::null();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_client_name(monado.root, self.id(), &mut string)
				.to_result()?
		};
		let c_string = unsafe { CStr::from_ptr(string) };
		c_string
			.to_str()
			.map_err(|_| MndResult::ErrorInvalidValue)
			.map(ToString::to_string)
	}
	fn state(&mut self) -> Result<FlagSet<ClientState>, MndResult> {
		let mut state = 0;
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_client_state(monado.root, self.id(), &mut state)
				.to_result()?
		};
		Ok(unsafe { FlagSet::new_unchecked(state) })
	}
	fn set_primary(&mut self) -> Result<(), MndResult> {
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_set_client_primary(monado.root, self.id())
				.to_result()
		}
	}
	fn set_focused(&mut self) -> Result<(), MndResult> {
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_set_client_focused(monado.root, self.id())
				.to_result()
		}
	}
	fn set_io_active(&mut self, active: bool) -> Result<(), MndResult> {
		let state = self.state()?;
		if state.contains(ClientState::ClientIoActive) != active {
			let monado = self.monado();
			unsafe {
				monado
					.api
					.mnd_root_toggle_client_io_active(monado.root, self.id())
					.to_result()?;
			}
		}
		Ok(())
	}
}

#[derive(Clone)]
pub struct Client<'m> {
	monado: &'m Monado,
	id: u32,
}
impl MonadoRef for Client<'_> {
	fn monado(&self) -> &Monado {
		self.monado
	}
}
impl ClientLogic for Client<'_> {
	fn id(&self) -> u32 {
		self.id
	}
}

#[cfg(feature = "rc")]
#[derive(Clone)]
pub struct ClientRc {
	monado: std::rc::Rc<Monado>,
	id: u32,
}
#[cfg(feature = "rc")]
impl MonadoRef for ClientRc {
	fn monado(&self) -> &Monado {
		self.monado.as_ref()
	}
}
#[cfg(feature = "rc")]
impl ClientLogic for ClientRc {
	fn id(&self) -> u32 {
		self.id
	}
}

#[cfg(feature = "arc")]
#[derive(Clone)]
pub struct ClientArc {
	monado: std::sync::Arc<Monado>,
	id: u32,
}
#[cfg(feature = "arc")]
impl MonadoRef for ClientArc {
	fn monado(&self) -> &Monado {
		self.monado.as_ref()
	}
}
#[cfg(feature = "arc")]
impl ClientLogic for ClientArc {
	fn id(&self) -> u32 {
		self.id
	}
}

pub trait DeviceLogic: MonadoRef {
	fn index(&self) -> u32;
	fn battery_status(&self) -> Result<BatteryStatus, MndResult> {
		let mut present: bool = Default::default();
		let mut charging: bool = Default::default();
		let mut charge: f32 = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_battery_status(
					monado.root,
					self.index(),
					&mut present,
					&mut charging,
					&mut charge,
				)
				.to_result()?;
		}
		Ok(BatteryStatus {
			present,
			charging,
			charge,
		})
	}
	fn serial(&self) -> Result<String, MndResult> {
		self.get_info_string(MndProperty::PropertySerialString)
	}
	fn get_info_bool(&self, property: MndProperty) -> Result<bool, MndResult> {
		let mut value: bool = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_info_bool(monado.root, self.index(), property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	fn get_info_u32(&self, property: MndProperty) -> Result<u32, MndResult> {
		let mut value: u32 = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_info_u32(monado.root, self.index(), property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	fn get_info_i32(&self, property: MndProperty) -> Result<i32, MndResult> {
		let mut value: i32 = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_info_i32(monado.root, self.index(), property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	fn get_info_f32(&self, property: MndProperty) -> Result<f32, MndResult> {
		let mut value: f32 = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_info_float(monado.root, self.index(), property, &mut value)
				.to_result()?
		}
		Ok(value)
	}
	fn get_info_string(&self, property: MndProperty) -> Result<String, MndResult> {
		let mut cstr_ptr = ptr::null_mut();
		let monado = self.monado();

		unsafe {
			monado
				.api
				.mnd_root_get_device_info_string(monado.root, self.index(), property, &mut cstr_ptr)
				.to_result()?
		}

		unsafe { Ok(CStr::from_ptr(cstr_ptr).to_string_lossy().to_string()) }
	}
	fn brightness(&self) -> Result<f32, MndResult> {
		let mut brightness: f32 = Default::default();
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_get_device_brightness(monado.root, self.index(), &mut brightness)
				.to_result()?;
		}
		Ok(brightness)
	}
	fn set_brightness(&self, brightness: f32, relative: bool) -> Result<(), MndResult> {
		let monado = self.monado();
		unsafe {
			monado
				.api
				.mnd_root_set_device_brightness(monado.root, self.index(), brightness, relative)
				.to_result()
		}
	}
}

#[derive(Clone)]
pub struct Device<'m> {
	monado: &'m Monado,
	pub index: u32,
	/// non-unique numeric representation of device name, see: xrt_device_name
	pub name_id: u32,
	pub name: String,
}
impl Device<'_> {}
impl Debug for Device<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Device")
			.field("id", &self.name_id)
			.field("name", &self.name)
			.finish()
	}
}
impl MonadoRef for Device<'_> {
	fn monado(&self) -> &Monado {
		self.monado
	}
}
impl DeviceLogic for Device<'_> {
	fn index(&self) -> u32 {
		self.index
	}
}

#[cfg(feature = "rc")]
#[derive(Clone)]
pub struct DeviceRc {
	monado: std::rc::Rc<Monado>,
	pub index: u32,
	/// non-unique numeric representation of device name, see: xrt_device_name
	pub name_id: u32,
	pub name: String,
}
#[cfg(feature = "rc")]
impl MonadoRef for DeviceRc {
	fn monado(&self) -> &Monado {
		self.monado.as_ref()
	}
}
#[cfg(feature = "rc")]
impl DeviceLogic for DeviceRc {
	fn index(&self) -> u32 {
		self.index
	}
}

#[cfg(feature = "arc")]
#[derive(Clone)]
pub struct DeviceArc {
	monado: std::sync::Arc<Monado>,
	pub index: u32,
	/// non-unique numeric representation of device name, see: xrt_device_name
	pub name_id: u32,
	pub name: String,
}
#[cfg(feature = "arc")]
impl MonadoRef for DeviceArc {
	fn monado(&self) -> &Monado {
		self.monado.as_ref()
	}
}
#[cfg(feature = "arc")]
impl DeviceLogic for DeviceArc {
	fn index(&self) -> u32 {
		self.index
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
		let _ = dbg!(device.name_id, &device.name, device.serial());
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
