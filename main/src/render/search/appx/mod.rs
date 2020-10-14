use std::{
    ffi::OsStr,
    fs::File,
    io::{prelude::*, Result},
    path::Path,
    process::Command,
    str::FromStr,
};

use com::{interfaces::IUnknown, runtime::create_instance, sys::GUID};
use image::{imageops::FilterType, DynamicImage};
use lazy_static::lazy_static;
use winapi::{
    shared::minwindef::DWORD, shared::minwindef::LPARAM, shared::windef::HWND,
    shared::winerror::HRESULT, um::winuser::EnumWindows, um::winuser::GetWindow,
    um::winuser::GetWindowThreadProcessId, um::winuser::GW_OWNER,
};

use super::{IndexEntry, LaunchTarget};
use crate::{
    bindings::windows::management::deployment::PackageManager,
    common::{Dll, ToWide},
};

#[com_interface("2e941141-7f97-4756-ba1d-9decde894a3d")]
pub trait IApplicationActivationManager: IUnknown {
    unsafe fn activate_application(
        &self,
        app_user_model_id: *const u16,
        arguments: *const u16,
        options: std::os::raw::c_int,
        process_id: *mut DWORD,
    ) -> HRESULT;
}

fn powershell<S: AsRef<str>>(cmd: S) -> Result<Vec<u8>> {
    const PS_PATH: &'static str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";

    let output = Command::new(PS_PATH)
        .arg("-NonInteractive")
        .arg("-WindowStyle")
        .arg("hidden")
        .arg(cmd.as_ref())
        .output()?;

    Ok(output.stdout)
}

fn list_start_apps() -> Vec<u8> {
    let cmd = "Get-StartApps \
        | ForEach-Object { \"{0} {1}{2} {3}\" -f $_.Name.Length,$_.Name,$_.AppId.Length,$_.AppId }";

    powershell(cmd).unwrap()
}

#[derive(Deserialize, Debug)]
struct PriInfo {
    #[serde(rename = "ResourceMap")]
    resource_map: ResourceMap,
}

#[derive(Deserialize, Debug)]
struct ResourceMap {
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "ResourceMapSubtree", default)]
    sub_trees: Vec<ResourceMapSubtree>,
}

#[derive(Deserialize, Debug)]
struct ResourceMapSubtree {
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "NamedResource", default)]
    resources: Vec<NamedResource>,
    #[serde(rename = "ResourceMapSubtree", default)]
    sub_trees: Vec<ResourceMapSubtree>,
}

#[derive(Deserialize, Debug)]
struct NamedResource {
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "uri")]
    uri: String,
    #[serde(rename = "Decision")]
    decision: Decision,
    #[serde(rename = "Candidate", default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize, Debug)]
struct Decision {
    #[serde(rename = "index")]
    index: usize,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    #[serde(rename = "type")]
    ty: String,
    #[serde(rename = "Value")]
    value: Option<String>,
    #[serde(rename = "QualifierSet")]
    qualifiers: Option<QualifierSet>,
}

impl Candidate {
    pub fn qualifier(&self, name: &str) -> Option<&str> {
        match &self.qualifiers {
            None => None,
            Some(list) => match list
                .qualifiers
                .iter()
                .find(|q| q.name.to_lowercase() == name)
            {
                None => None,
                Some(q) => Some(&q.value),
            },
        }
    }

    pub fn qualifer_as<V: FromStr>(&self, name: &str) -> Option<V> {
        self.qualifier(name).and_then(|q| match V::from_str(&q) {
            Ok(v) => Some(v),
            Err(_) => None,
        })
    }

    pub fn match_str(&self, name: &str, value: &str) -> bool {
        match self.qualifier(name) {
            Some(v) => v.to_lowercase() == value,
            None => false,
        }
    }

    pub fn match_atleast<V: FromStr + PartialOrd>(&self, name: &str, value: V) -> bool {
        match self.qualifer_as::<V>(name) {
            None => false,
            Some(v) => v >= value,
        }
    }
}

#[derive(Deserialize, Debug)]
struct QualifierSet {
    #[serde(rename = "Qualifier", default)]
    qualifiers: Vec<Qualifier>,
}

#[derive(Deserialize, Debug)]
struct Qualifier {
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "value")]
    value: String,
}

#[derive(Deserialize, Debug)]
struct Package {
    #[serde(rename = "Properties")]
    properties: PackageProperties,
    #[serde(rename = "Applications")]
    applications: Applications,
}

#[derive(Deserialize, Debug)]
struct PackageProperties {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Logo")]
    logo: String,
}

#[derive(Deserialize, Debug)]
struct Applications {
    #[serde(rename = "Application", default)]
    applications: Vec<Application>,
}

#[derive(Deserialize, Debug)]
struct Application {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Executable")]
    executable: Option<String>,
    #[serde(rename = "EntryPoint")]
    entry_point: Option<String>,
    #[serde(rename = "VisualElements")]
    visual_elements: VisualElements,
}

#[derive(Deserialize, Debug)]
struct VisualElements {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Square44x44Logo")]
    square_44x44_logo: String,
    #[serde(rename = "Square150x150Logo")]
    square_150x150_logo: String,
}

#[allow(non_upper_case_globals)]
fn load_pri<P: AsRef<OsStr>>(path: P) -> PriInfo {
    // TODO: this function is slow //
    lazy_static! {
        static ref mrmsupport: Dll = Dll::load("mrmsupport.dll").unwrap();
        static ref MrmDumpPriFileInMemory: unsafe extern "system" fn(
            *const u16,
            *const u16,
            std::os::raw::c_uint,
            *mut *mut u8,
            *mut std::os::raw::c_ulong,
        ) -> HRESULT = unsafe {
            std::mem::transmute(mrmsupport.get_function("MrmDumpPriFileInMemory").unwrap())
        };
        static ref MrmFreeMemory: unsafe extern "system" fn(*mut u8) -> HRESULT =
            unsafe { std::mem::transmute(mrmsupport.get_function("MrmFreeMemory").unwrap()) };
    }

    let raw = path.as_ref().to_wide();
    let mut out_data: *mut u8 = std::ptr::null_mut();
    let mut out_size = 0;
    let data = unsafe {
        MrmDumpPriFileInMemory(
            raw.as_ptr(),
            std::ptr::null_mut(),
            1,
            &mut out_data,
            &mut out_size,
        );
        std::slice::from_raw_parts_mut(out_data, out_size as usize)
    };

    let xml = std::str::from_utf8(data).unwrap();
    let parsed = quick_xml::de::from_str(xml).unwrap();

    unsafe { MrmFreeMemory(out_data) };

    parsed
}

fn find_resource<'a>(pris: &'a [PriInfo], name: &str, white: bool) -> Option<&'a str> {
    let tail = "/".to_owned() + &name.to_lowercase();

    fn find_resource<'a>(tree: &'a ResourceMapSubtree, tail: &str, white: bool) -> Option<&'a str> {
        for rsrc in &tree.resources {
            if rsrc.uri.to_lowercase().ends_with(&tail) {
                // println!("{}", rsrc.uri);
                // for candidate in &rsrc.candidates {
                //     if let Some(value) = &candidate.value {
                //         println!("  {}", value);
                //     }
                // }

                let filtered: Vec<_> = rsrc
                    .candidates
                    .iter()
                    .filter(|c| c.value.is_some() && (!white || c.match_str("contrast", "white")))
                    .collect();

                let mut pass: Vec<_> = filtered
                    .iter()
                    .filter(|c| c.match_str("targetsize", "64"))
                    .collect();
                if pass.len() > 0 {
                    pass.sort_by(|a, b| {
                        let a = a.qualifiers.as_ref().map_or(0, |l| l.qualifiers.len());
                        let b = b.qualifiers.as_ref().map_or(0, |l| l.qualifiers.len());
                        a.cmp(&b)
                    });

                    return Some(pass[0].value.as_ref().unwrap());
                }

                let mut pass: Vec<_> = filtered
                    .iter()
                    .filter(|c| c.match_atleast("targetsize", 64))
                    .collect();
                if pass.len() > 0 {
                    pass.sort_by(|a, b| {
                        let a = a.qualifiers.as_ref().map_or(0, |l| l.qualifiers.len());
                        let b = b.qualifiers.as_ref().map_or(0, |l| l.qualifiers.len());
                        a.cmp(&b)
                    });
                    pass.sort_by(|a, b| {
                        let a: usize = a.qualifer_as("targetsize").unwrap_or(0);
                        let b: usize = b.qualifer_as("targetsize").unwrap_or(0);
                        a.cmp(&b)
                    });

                    return Some(pass[0].value.as_ref().unwrap());
                }

                // println!("fallback");
                for candidate in filtered.iter().rev() {
                    if let Some(value) = &candidate.value {
                        return Some(value);
                    }
                }
            }
        }

        for subtree in &tree.sub_trees {
            if let Some(value) = find_resource(subtree, &tail, white) {
                return Some(value);
            }
        }

        None
    }

    for pri in pris {
        for tree in &pri.resource_map.sub_trees {
            if let Some(value) = find_resource(tree, &tail, white) {
                return Some(value);
            }
        }
    }

    None
}

fn split_row(src: &str) -> impl Iterator<Item = &str> {
    let mut remain = src;
    let mut vec = vec![];
    while remain.len() > 0 {
        let i = remain.find(' ').unwrap();
        let len: usize = remain[0..i].parse().unwrap();
        let value = &remain[i + 1..i + 1 + len];
        remain = &remain[i + 1 + len..];
        vec.push(value);
    }
    vec.into_iter()
}

pub fn luminance(i: &[f64; 3]) -> f64 {
    let r = if i[0] < 0.03928 {
        i[0] / 12.92
    } else {
        ((i[0] + 0.055) / 1.055).powf(2.4)
    };
    let g = if i[1] < 0.03928 {
        i[1] / 12.92
    } else {
        ((i[1] + 0.055) / 1.055).powf(2.4)
    };
    let b = if i[2] < 0.03928 {
        i[2] / 12.92
    } else {
        ((i[2] + 0.055) / 1.055).powf(2.4)
    };

    r * 0.2129 + g * 0.7152 + b * 0.0722
}

pub fn index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let pm = PackageManager::new().unwrap();

    let raw = list_start_apps();
    let packages = std::str::from_utf8(&raw)
        .unwrap()
        .split('\n')
        .map(|line| line.trim())
        .filter(|line| line.len() > 0)
        .map(|line| split_row(line))
        .map(|mut i| (i.next().unwrap(), i.next().unwrap()))
        .filter_map(|(app_name, launch_id)| {
            // launch_id has the form "family_name!application_id"
            let i = launch_id.find('!')?;
            let family_name = &launch_id[..i];
            let app_id = &launch_id[i + 1..];

            let packages: Vec<_> = crate::nonfatal(|| {
                let packages =
                    pm.find_packages_by_user_security_id_package_family_name("", family_name)?;
                Ok(packages.into_iter().collect())
            })?;

            assert!(packages.len() == 1);

            let path = crate::nonfatal(|| {
                let path = packages[0].installed_location()?.path()?.to_string();
                Ok(path)
            })?;

            let path = Path::new(&path);

            let manifest: Package = crate::nonfatal(|| {
                let mut manifest = vec![];
                let manifest_path = path.join("AppxManifest.xml");
                File::open(manifest_path)?.read_to_end(&mut manifest)?;
                let package = quick_xml::de::from_reader(&manifest as &[u8])?;
                Ok(package)
            })?;

            let app = manifest
                .applications
                .applications
                .iter()
                .find(|a| a.id == app_id)?;

            let pris: Vec<_> = ["resources.pri", "pris/resources.en-US.pri"]
                .iter()
                .filter_map(|relative| {
                    let path = path.join(relative);
                    match path.exists() {
                        true => Some(load_pri(path)),
                        false => None,
                    }
                })
                .collect();

            let logo_asset = &app.visual_elements.square_44x44_logo;
            let logo_path = path.join(logo_asset);
            let logo_path = if logo_path.exists() {
                logo_path
            } else {
                let uri_tail = logo_asset.replace('\\', "/");
                let found = find_resource(&pris, &uri_tail, false)?;
                let logo_path = path.join(found);

                let valid = crate::nonfatal(|| {
                    let src = image::open(&logo_path)?;
                    let src = src.resize(64, 64, FilterType::CatmullRom);

                    let mut full = image::RgbaImage::from_pixel(64, 64, [0; 4].into());
                    image::imageops::overlay(&mut full, &src, 0, 0);

                    let mut sum = [0.0; 3];
                    let mut total_alpha = 0.0;
                    for (_, _, pixel) in full.enumerate_pixels() {
                        sum[0] += pixel[0] as f64 * pixel[3] as f64;
                        sum[1] += pixel[1] as f64 * pixel[3] as f64;
                        sum[2] += pixel[2] as f64 * pixel[3] as f64;
                        total_alpha += pixel[3] as f64;
                    }
                    sum[0] /= total_alpha * 255.0;
                    sum[1] /= total_alpha * 255.0;
                    sum[2] /= total_alpha * 255.0;

                    let image = luminance(&sum);
                    let white = luminance(&[0.0; 3]);
                    let ratio = (image + 0.05) / (white + 0.05);

                    Ok(ratio < 15.0)
                })?;

                if !valid {
                    if let Some(white) = find_resource(&pris, &uri_tail, true) {
                        path.join(white)
                    } else {
                        logo_path
                    }
                } else {
                    logo_path
                }
            };

            let keys = [app_name];

            let details = family_name.to_string();

            let display_icon = crate::nonfatal(|| {
                let src = image::open(logo_path)?;
                let src = src.resize(64, 64, FilterType::CatmullRom);

                let mut out = image::RgbaImage::from_pixel(64, 64, [0; 4].into());
                image::imageops::overlay(&mut out, &src, 0, 0);

                Ok(DynamicImage::ImageRgba8(out))
            });

            let index = IndexEntry::new(keys.iter());

            let launch_id = launch_id.to_owned();
            let launch = Box::new(move || {
                const CLSID: GUID = GUID {
                    data1: 0x45BA127D,
                    data2: 0x10A8,
                    data3: 0x46EA,
                    data4: [0x8A, 0xB7, 0x56, 0xEA, 0x90, 0x78, 0x94, 0x3C],
                };

                let raw = launch_id.to_wide();

                unsafe {
                    std::thread::spawn(move || {
                        let am =
                            create_instance::<dyn IApplicationActivationManager>(&CLSID).unwrap();

                        let mut process_id = 0;
                        am.activate_application(raw.as_ptr(), std::ptr::null(), 0, &mut process_id);

                        std::thread::sleep(std::time::Duration::from_millis(200));

                        EnumWindows(Some(enum_windows_helper), process_id as isize);
                    });
                }

                unsafe extern "system" fn enum_windows_helper(win: HWND, l: LPARAM) -> i32 {
                    let original = l as u32;

                    let mut process_id = 0;
                    GetWindowThreadProcessId(win, &mut process_id);

                    if original == process_id {
                        let parent = GetWindow(win, GW_OWNER);
                        if parent.is_null() {
                            crate::common::focus_window(win);
                            return 0;
                        }

                        let mut parent_process_id = 0;
                        GetWindowThreadProcessId(parent, &mut parent_process_id);
                        if parent_process_id != process_id {
                            crate::common::focus_window(parent);
                            return 0;
                        }
                    }

                    1
                }
            });

            let target = LaunchTarget {
                details,
                display_icon,
                launch,
            };

            Some((index, target))
        });

    let packages: Vec<_> = packages.collect();
    packages.into_iter()
}
