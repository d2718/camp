/*!
Populating the local "fake production" environment with sufficient data
to allow some experimentation.

Fake production data can be found in `crate_root/fakeprod_data`.
*/

use camp::*
use camp::{
    user::{Admin, Boss, Student, Teacher, User},
};

fn csv_file_to_staff<R: Read>(r: R) -> Result<Vec<User>, String> {
    
}