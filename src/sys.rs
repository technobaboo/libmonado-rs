use dlopen2::wrapper::WrapperApi;
use std::ffi::c_void;
use std::fmt::Debug;

#[repr(i32)]
#[doc = " Result codes for operations, negative are errors, zero or positives are\n success."]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MndResult {
	Success = 0,
	ErrorInvalidVersion = -1,
	ErrorInvalidValue = -2,
	ErrorConnectingFailed = -3,
	ErrorOperationFailed = -4,
	ErrorRecenteringNotSupported = -5,
	ErrorInvalidProperty = -6,
}
impl MndResult {
	pub fn to_result(self) -> Result<(), MndResult> {
		if self == MndResult::Success {
			Ok(())
		} else {
			Err(self)
		}
	}
}

flagset::flags! {
	#[doc = " Bitflags for client application state."]
	pub enum ClientState: u32 {
		ClientPrimaryApp = 1,
		ClientSessionActive = 2,
		ClientSessionVisible = 4,
		ClientSessionFocused = 8,
		ClientSessionOverlay = 16,
		ClientIoActive = 32,
	}
}

#[repr(i32)]
#[doc = " A property to get from a thing (currently only devices)."]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MndProperty {
	PropertyNameString = 1,
	PropertySerialString = 2,
}


#[doc = " Opaque type for libmonado state"]
pub type MndRootPtr = *mut c_void;

#[derive(WrapperApi)]
pub struct MonadoApi {
	mnd_api_get_version:
		unsafe extern "C" fn(out_major: *mut u32, out_minor: *mut u32, out_patch: *mut u32),
	mnd_root_create: unsafe extern "C" fn(out_root: *mut MndRootPtr) -> MndResult,
	mnd_root_destroy: unsafe extern "C" fn(out_root: *mut MndRootPtr),
	mnd_root_update_client_list: unsafe extern "C" fn(root: MndRootPtr) -> MndResult,
	mnd_root_get_number_clients:
		unsafe extern "C" fn(root: MndRootPtr, out_num: *mut u32) -> MndResult,
	mnd_root_get_client_id_at_index:
		unsafe extern "C" fn(root: MndRootPtr, index: u32, out_client_id: *mut u32) -> MndResult,
	mnd_root_get_client_name: unsafe extern "C" fn(
		root: MndRootPtr,
		client_id: u32,
		out_name: *mut *const ::std::os::raw::c_char,
	) -> MndResult,
	mnd_root_get_client_state:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32, out_flags: *mut u32) -> MndResult,
	mnd_root_set_client_primary:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	mnd_root_set_client_focused:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	mnd_root_toggle_client_io_active:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	mnd_root_get_device_count:
		unsafe extern "C" fn(root: MndRootPtr, out_device_count: *mut u32) -> MndResult,
	mnd_root_get_device_info: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		out_index: *mut u32,
		out_dev_name: *mut *const ::std::os::raw::c_char,
	) -> MndResult,
	mnd_root_get_device_from_role: unsafe extern "C" fn(
		root: MndRootPtr,
		role_name: *const ::std::os::raw::c_char,
		out_index: *mut i32,
	) -> MndResult,
	mnd_root_recenter_local_spaces: unsafe extern "C" fn(root: MndRootPtr) -> MndResult,
	mnd_root_get_device_info_bool: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		mnd_property_t: MndProperty,
		out_bool: *mut bool,
	) -> MndResult,
	mnd_root_get_device_info_i32: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		mnd_property_t: MndProperty,
		out_i32: *mut i32,
	) -> MndResult,
	mnd_root_get_device_info_u32: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		mnd_property_t: MndProperty,
		out_u32: *mut u32,
	) -> MndResult,
	mnd_root_get_device_info_float: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		mnd_property_t: MndProperty,
		out_float: *mut f32,
	) -> MndResult,
	mnd_root_get_device_info_string: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		mnd_property_t: MndProperty,
		out_string: *const ::std::os::raw::c_char,
	) -> MndResult,
}
