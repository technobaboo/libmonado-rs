use dlopen2::wrapper::WrapperApi;
use flagset::flags;
use std::fmt::Debug;
use std::os::raw::c_char;
use std::{ffi::c_void, fmt::Display};

use crate::space::{MndPose, ReferenceSpaceType};

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
	ErrorInvalidOperation = -7,
	ErrorUnsupportedOperation = -8,
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

impl std::error::Error for MndResult {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		None
	}
	fn cause(&self) -> Option<&dyn std::error::Error> {
		None
	}
	fn description(&self) -> &str {
		"std::error::Error::description() is deprecated, use the Display impl instead."
	}
}

impl Display for MndResult {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct MndColorHsv {
	pub h: f32,
	pub s: f32,
	pub v: f32,
}

flags! {
	#[doc = " Bitflags for client application state."]
	pub enum ClientState: u32 {
		ClientPrimaryApp = 1,
		ClientSessionActive = 2,
		ClientSessionVisible = 4,
		ClientSessionFocused = 8,
		ClientSessionOverlay = 16,
		ClientIoActive = 32,
		ClientPosesBlocked = 64,
		ClientHtBlocked = 128,
		ClientInputsBlocked = 256,
		ClientOutputsBlocked = 512,
	}
}

flags! {
	#[doc = " Bitflags for IO blocking."]
	#[repr(u32)]
	pub enum BlockFlags: u32 {
		None = 0,
		BlockPoses = 1,
		BlockHt = 2,
		BlockInputs = 4,
		BlockOutputs = 8,
	}
}

#[repr(i32)]
#[doc = " A property to get from a thing (currently only devices)."]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MndProperty {
	PropertyNameString = 0,
	PropertySerialString = 1,
	PropertyTrackingOriginU32 = 2,
	PropertySupportsPositionBool = 3,
	PropertySupportsOrientationBool = 4,
	PropertySupportsBrightnessBool = 5,
}

#[doc = " Opaque type for libmonado state"]
pub type MndRootPtr = *mut c_void;

#[derive(WrapperApi)]
pub struct MonadoApi {
	// === API version 1.0 ===

	/// Get the API version (not Monado itself).
	mnd_api_get_version:
		unsafe extern "C" fn(out_major: *mut u32, out_minor: *mut u32, out_patch: *mut u32),
	/// Create libmonado state and connect to service.
	mnd_root_create: unsafe extern "C" fn(out_root: *mut MndRootPtr) -> MndResult,
	/// Destroy libmonado state, disconnecting from the service.
	mnd_root_destroy: unsafe extern "C" fn(out_root: *mut MndRootPtr),
	/// Update the local cached copy of the client list.
	mnd_root_update_client_list: unsafe extern "C" fn(root: MndRootPtr) -> MndResult,
	/// Get the number of active clients.
	mnd_root_get_number_clients:
		unsafe extern "C" fn(root: MndRootPtr, out_num: *mut u32) -> MndResult,
	/// Get the id from the current client list at the given index.
	mnd_root_get_client_id_at_index:
		unsafe extern "C" fn(root: MndRootPtr, index: u32, out_client_id: *mut u32) -> MndResult,
	/// Get the name of the client with the given id.
	mnd_root_get_client_name: unsafe extern "C" fn(
		root: MndRootPtr,
		client_id: u32,
		out_name: *mut *const ::std::os::raw::c_char,
	) -> MndResult,
	/// Get the state flags of the client with the given id.
	mnd_root_get_client_state:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32, out_flags: *mut u32) -> MndResult,
	/// Set the client with the given id as "primary".
	mnd_root_set_client_primary:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	/// Set the client with the given id as "focused".
	mnd_root_set_client_focused:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	/// Toggle IO activity for a client. Deprecated in version 1.6.
	mnd_root_toggle_client_io_active:
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32) -> MndResult,
	/// Get the number of devices.
	mnd_root_get_device_count:
		unsafe extern "C" fn(root: MndRootPtr, out_device_count: *mut u32) -> MndResult,
	/// Get device info at the given index. Deprecated in version 1.2.
	mnd_root_get_device_info: unsafe extern "C" fn(
		root: MndRootPtr,
		device_index: u32,
		out_index: *mut u32,
		out_dev_name: *mut *const ::std::os::raw::c_char,
	) -> MndResult,
	/// Get the device index associated with a given role name.
	mnd_root_get_device_from_role: unsafe extern "C" fn(
		root: MndRootPtr,
		role_name: *const ::std::os::raw::c_char,
		out_index: *mut i32,
	) -> MndResult,

	// === API version 1.1 ===

	/// Trigger a recenter of the local spaces. Since API version 1.1.
	mnd_root_recenter_local_spaces: Option<
		unsafe extern "C" fn(root: MndRootPtr) -> MndResult,
	>,

	// === API version 1.2 ===

	/// Get boolean property for a device. Since API version 1.2.
	mnd_root_get_device_info_bool: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			mnd_property_t: MndProperty,
			out_bool: *mut bool,
		) -> MndResult,
	>,
	/// Get i32 property for a device. Since API version 1.2.
	mnd_root_get_device_info_i32: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			mnd_property_t: MndProperty,
			out_i32: *mut i32,
		) -> MndResult,
	>,
	/// Get u32 property for a device. Since API version 1.2.
	mnd_root_get_device_info_u32: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			mnd_property_t: MndProperty,
			out_u32: *mut u32,
		) -> MndResult,
	>,
	/// Get float property for a device. Since API version 1.2.
	mnd_root_get_device_info_float: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			mnd_property_t: MndProperty,
			out_float: *mut f32,
		) -> MndResult,
	>,
	/// Get string property for a device. Since API version 1.2.
	mnd_root_get_device_info_string: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			mnd_property_t: MndProperty,
			out_string: *mut *mut ::std::os::raw::c_char,
		) -> MndResult,
	>,

	// === API version 1.3 ===

	/// Get the current offset of a reference space. Since API version 1.3.
	mnd_root_get_reference_space_offset: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			type_: ReferenceSpaceType,
			out_offset: *mut MndPose,
		) -> MndResult,
	>,
	/// Apply an offset to a reference space. Since API version 1.3.
	mnd_root_set_reference_space_offset: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			type_: ReferenceSpaceType,
			offset: *const MndPose,
		) -> MndResult,
	>,
	/// Read the current offset of a tracking origin. Since API version 1.3.
	mnd_root_get_tracking_origin_offset: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			origin_id: u32,
			out_offset: *mut MndPose,
		) -> MndResult,
	>,
	/// Apply an offset to a tracking origin. Since API version 1.3.
	mnd_root_set_tracking_origin_offset: Option<
		unsafe extern "C" fn(root: MndRootPtr, origin_id: u32, offset: *const MndPose) -> MndResult,
	>,
	/// Get the number of tracking origins. Since API version 1.3.
	mnd_root_get_tracking_origin_count: Option<
		unsafe extern "C" fn(root: MndRootPtr, out_track_count: *mut u32) -> MndResult,
	>,
	/// Get the name of a tracking origin. Since API version 1.3.
	mnd_root_get_tracking_origin_name: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			origin_id: u32,
			out_string: *mut *const c_char,
		) -> MndResult,
	>,

	// === API version 1.4 ===

	/// Get battery status of a device. Since API version 1.4.
	mnd_root_get_device_battery_status: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			out_present: *mut bool,
			out_charging: *mut bool,
			out_charge: *mut f32,
		) -> MndResult,
	>,

	// === API version 1.5 ===

	/// Get current brightness of a display device. Since API version 1.5.
	mnd_root_get_device_brightness: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			out_brightness: *mut f32,
		) -> MndResult,
	>,
	/// Set the display brightness. Since API version 1.5.
	mnd_root_set_device_brightness: Option<
		unsafe extern "C" fn(
			root: MndRootPtr,
			device_index: u32,
			brightness: f32,
			relative: bool,
		) -> MndResult,
	>,

	// === API version 1.6 ===

	/// Block certain types of IO for a client. Since API version 1.6.
	mnd_root_set_client_io_blocks: Option<
		unsafe extern "C" fn(root: MndRootPtr, client_id: u32, block_flags: u32) -> MndResult,
	>,
	
	/// Set chroma key params for any base application opaque projection layer. Since API version 1.6.
	mnd_root_set_chroma_key_params: Option<
    	unsafe extern "C" fn(
			root: MndRootPtr,
			min: MndColorHsv,
			max: MndColorHsv,
			curve: f32,
			despill: f32,
		) -> MndResult,
	>,
}
