use crate::{sys::MndResult, Monado};
use std::{
	ffi::{c_char, CStr},
	vec,
};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct MndPose {
	pub orientation: MndQuaternion,
	pub position: MndVector3,
}
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct MndQuaternion {
	pub x: f32,
	pub y: f32,
	pub z: f32,
	pub w: f32,
}
impl From<MndQuaternion> for mint::Quaternion<f32> {
	fn from(q: MndQuaternion) -> Self {
		mint::Quaternion {
			v: mint::Vector3 {
				x: q.x,
				y: q.y,
				z: q.z,
			},
			s: q.w,
		}
	}
}
impl From<mint::Quaternion<f32>> for MndQuaternion {
	fn from(q: mint::Quaternion<f32>) -> Self {
		Self {
			x: q.v.x,
			y: q.v.y,
			z: q.v.z,
			w: q.s,
		}
	}
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct MndVector3 {
	pub x: f32,
	pub y: f32,
	pub z: f32,
}
impl From<MndVector3> for mint::Vector3<f32> {
	fn from(v: MndVector3) -> Self {
		mint::Vector3 {
			x: v.x,
			y: v.y,
			z: v.z,
		}
	}
}
impl From<mint::Vector3<f32>> for MndVector3 {
	fn from(v: mint::Vector3<f32>) -> Self {
		Self {
			x: v.x,
			y: v.y,
			z: v.z,
		}
	}
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSpaceType {
	View = 0,
	Local = 1,
	LocalFloor = 2,
	Stage = 3,
	Unbounded = 4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
	pub position: mint::Vector3<f32>,
	pub orientation: mint::Quaternion<f32>,
}
impl From<MndPose> for Pose {
	fn from(value: MndPose) -> Self {
		Self {
			position: value.position.into(),
			orientation: value.orientation.into(),
		}
	}
}
impl From<Pose> for MndPose {
	fn from(value: Pose) -> Self {
		Self {
			orientation: value.orientation.into(),
			position: value.position.into(),
		}
	}
}

impl Monado {
	pub fn tracking_origins(
		&self,
	) -> Result<impl IntoIterator<Item = TrackingOrigin<'_>>, MndResult> {
		let mut count = 0;
		unsafe {
			self.api
				.mnd_root_get_tracking_origin_count(self.root, &mut count)
				.to_result()?
		};
		let mut tracking_origins: Vec<Option<TrackingOrigin>> =
			vec::from_elem(None, count as usize);
		for (id, origin) in tracking_origins.iter_mut().enumerate() {
			let mut c_name: *const c_char = std::ptr::null_mut();
			unsafe {
				self.api
					.mnd_root_get_tracking_origin_name(self.root, id as u32, &mut c_name)
					.to_result()?
			};
			let name = unsafe {
				CStr::from_ptr(c_name)
					.to_str()
					.map_err(|_| MndResult::ErrorInvalidValue)?
					.to_owned()
			};
			origin.replace(TrackingOrigin {
				monado: self,
				id: id as u32,
				name,
			});
		}
		Ok(tracking_origins.into_iter().flatten())
	}

	pub fn get_reference_space_offset(
		&self,
		space_type: ReferenceSpaceType,
	) -> Result<Pose, MndResult> {
		let mut mnd_pose = MndPose::default();
		unsafe {
			self.api
				.mnd_root_get_reference_space_offset(self.root, space_type, &mut mnd_pose)
				.to_result()?;
		}
		Ok(mnd_pose.into())
	}
	pub fn set_reference_space_offset(
		&self,
		space_type: ReferenceSpaceType,
		pose: Pose,
	) -> Result<(), MndResult> {
		unsafe {
			self.api
				.mnd_root_set_reference_space_offset(self.root, space_type, &pose.into())
				.to_result()
		}
	}
}

#[derive(Clone)]
pub struct TrackingOrigin<'m> {
	monado: &'m Monado,
	pub id: u32,
	pub name: String,
}
impl TrackingOrigin<'_> {
	pub fn get_offset(&self) -> Result<Pose, MndResult> {
		let mut mnd_pose = MndPose::default();
		unsafe {
			self.monado
				.api
				.mnd_root_get_tracking_origin_offset(self.monado.root, self.id, &mut mnd_pose)
				.to_result()?;
		}
		Ok(mnd_pose.into())
	}
	pub fn set_offset(&self, pose: Pose) -> Result<(), MndResult> {
		unsafe {
			self.monado
				.api
				.mnd_root_set_tracking_origin_offset(self.monado.root, self.id, &pose.into())
				.to_result()
		}
	}
}

#[test]
fn test_spaces() {
	let monado = Monado::auto_connect().unwrap();
	for tracking_origin in monado.tracking_origins().unwrap() {
		dbg!(
			tracking_origin.id,
			&tracking_origin.name,
			tracking_origin.get_offset().unwrap()
		);
		println!();
	}
	let test_reference_space = |space_type| -> Result<Pose, MndResult> {
		let offset = monado.get_reference_space_offset(space_type)?;
		monado.set_reference_space_offset(space_type, offset)?;
		Ok(offset)
	};

	let _ = dbg!(test_reference_space(ReferenceSpaceType::Local));
	let _ = dbg!(test_reference_space(ReferenceSpaceType::LocalFloor));
	let _ = dbg!(test_reference_space(ReferenceSpaceType::Stage));
	let _ = dbg!(test_reference_space(ReferenceSpaceType::Unbounded));
	let _ = dbg!(test_reference_space(ReferenceSpaceType::View));
}
