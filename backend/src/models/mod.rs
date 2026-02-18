// 数据模型层模块

pub mod comment;
pub mod course;
pub mod favorite;
pub mod image;
pub mod like;
pub mod notification;
pub mod rating;
pub mod resource;
pub mod teacher;
pub mod user;

// 模型导出供其他模块使用
#[allow(unused_imports)]
pub use comment::*;
#[allow(unused_imports)]
pub use course::*;
#[allow(unused_imports)]
pub use favorite::*;
#[allow(unused_imports)]
pub use image::*;
#[allow(unused_imports)]
pub use like::*;
#[allow(unused_imports)]
pub use notification::*;
#[allow(unused_imports)]
pub use rating::*;
#[allow(unused_imports)]
pub use resource::*;
#[allow(unused_imports)]
pub use teacher::*;
#[allow(unused_imports)]
pub use user::*;
