//! Launch with MSFS via the simulator's `exe.xml` add-on list (port of `AppAutoStart.cs`).

use std::path::{Path, PathBuf};
use xmltree::{Element, EmitterConfig, XMLNode};

pub const ADDON_NAME: &str = "MSFS Popout Panel Manager";

/// Decode the file content: UTF-8 (with optional BOM) or, failing that, Latin-1.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Remove a leading `<?xml ... ?>` declaration (its encoding attribute may lie).
fn strip_declaration(s: &str) -> &str {
    let t = s.trim_start();
    if t.starts_with("<?xml")
        && let Some(end) = t.find("?>")
    {
        return t[end + 2..].trim_start();
    }
    t
}

pub fn parse(bytes: &[u8]) -> Result<Element, String> {
    let text = decode(bytes);
    Element::parse(strip_declaration(&text).as_bytes()).map_err(|e| e.to_string())
}

pub fn serialize(root: &Element) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let cfg = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("  ")
        .write_document_declaration(true);
    root.write_with_config(&mut out, cfg)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn text_element(name: &str, text: &str) -> Element {
    let mut e = Element::new(name);
    e.children.push(XMLNode::Text(text.to_string()));
    e
}

fn addon_name(e: &Element) -> Option<String> {
    e.get_child("Name")
        .and_then(|n| n.get_text())
        .map(|t| t.trim().to_string())
}

pub fn has_entry(root: &Element) -> bool {
    root.children.iter().any(|c| {
        c.as_element().is_some_and(|e| {
            e.name == "Launch.Addon" && addon_name(e).as_deref() == Some(ADDON_NAME)
        })
    })
}

fn new_entry(exe_path: &str) -> Element {
    let mut e = Element::new("Launch.Addon");
    e.children
        .push(XMLNode::Element(text_element("Name", ADDON_NAME)));
    e.children
        .push(XMLNode::Element(text_element("Disabled", "false")));
    e.children
        .push(XMLNode::Element(text_element("Path", exe_path)));
    e
}

/// Add (or replace) our entry, preserving every other entry.
/// An existing entry keeps its `CommandLine`.
pub fn add_entry(root: &mut Element, exe_path: &str) {
    let existing = root.children.iter().position(|c| {
        c.as_element().is_some_and(|e| {
            e.name == "Launch.Addon" && addon_name(e).as_deref() == Some(ADDON_NAME)
        })
    });
    let mut entry = new_entry(exe_path);
    match existing {
        Some(i) => {
            if let Some(old) = root.children[i].as_element()
                && let Some(cl) = old.get_child("CommandLine")
            {
                entry.children.push(XMLNode::Element(cl.clone()));
            }
            root.children[i] = XMLNode::Element(entry);
        }
        None => root.children.push(XMLNode::Element(entry)),
    }
}

pub fn remove_entry(root: &mut Element) -> bool {
    let before = root.children.len();
    root.children.retain(|c| {
        !c.as_element().is_some_and(|e| {
            e.name == "Launch.Addon" && addon_name(e).as_deref() == Some(ADDON_NAME)
        })
    });
    root.children.len() != before
}

/// A fresh document for installs that have no `exe.xml` yet.
pub fn new_document(exe_path: &str) -> Element {
    let mut root = Element::new("SimBase.Document");
    root.attributes.insert("Type".into(), "SimConnect".into());
    root.attributes.insert("version".into(), "1,0".into());
    root.children
        .push(XMLNode::Element(text_element("Descr", "SimConnect")));
    root.children
        .push(XMLNode::Element(text_element("Filename", "SimConnect.xml")));
    root.children
        .push(XMLNode::Element(text_element("Disabled", "False")));
    root.children.push(XMLNode::Element(new_entry(exe_path)));
    root
}

// ---------------------------------------------------------------------------
// File system wrappers
// ---------------------------------------------------------------------------

/// Locate `exe.xml`: Microsoft Store install first, then Steam.
pub fn exe_xml_path() -> Option<PathBuf> {
    let store = dirs::data_local_dir().map(|d| {
        d.join("Packages")
            .join("Microsoft.FlightSimulator_8wekyb3d8bbwe")
            .join("LocalCache")
    });
    let steam = dirs::config_dir().map(|d| d.join("Microsoft Flight Simulator"));
    [store, steam]
        .into_iter()
        .flatten()
        .find(|d| d.is_dir())
        .map(|d| d.join("exe.xml"))
}

pub fn is_auto_start() -> bool {
    let Some(path) = exe_xml_path() else {
        return false;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    parse(&bytes).map(|r| has_entry(&r)).unwrap_or(false)
}

fn current_exe_string() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

pub fn activate() -> Result<(), String> {
    let path = exe_xml_path().ok_or("MSFS installation folder not found")?;
    let exe = current_exe_string().ok_or("cannot determine executable path")?;
    activate_at(&path, &exe)
}

pub fn activate_at(path: &Path, exe: &str) -> Result<(), String> {
    if path.exists() {
        let backup = PathBuf::from(format!("{}.backup", path.display()));
        if !backup.exists() {
            std::fs::copy(path, &backup).map_err(|e| e.to_string())?;
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut root = parse(&bytes)?;
        add_entry(&mut root, exe);
        std::fs::write(path, serialize(&root)?).map_err(|e| e.to_string())
    } else {
        std::fs::write(path, serialize(&new_document(exe))?).map_err(|e| e.to_string())
    }
}

pub fn deactivate() -> Result<(), String> {
    let Some(path) = exe_xml_path() else {
        return Ok(());
    };
    deactivate_at(&path)
}

pub fn deactivate_at(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut root = parse(&bytes)?;
    if remove_entry(&mut root) {
        std::fs::write(path, serialize(&root)?).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="Windows-1252"?>
<SimBase.Document Type="SimConnect" version="1,0">
  <Descr>SimConnect</Descr>
  <Filename>SimConnect.xml</Filename>
  <Disabled>False</Disabled>
  <Launch.Addon>
    <Name>Other Tool</Name>
    <Disabled>False</Disabled>
    <Path>C:\Tools\other.exe</Path>
    <CommandLine>-flag</CommandLine>
  </Launch.Addon>
</SimBase.Document>"#;

    fn count_addons(root: &Element) -> usize {
        root.children
            .iter()
            .filter(|c| c.as_element().is_some_and(|e| e.name == "Launch.Addon"))
            .count()
    }

    #[test]
    fn adds_entry_and_preserves_others() {
        let mut root = parse(SAMPLE.as_bytes()).unwrap();
        assert!(!has_entry(&root));
        add_entry(&mut root, r"C:\popm\popm.exe");
        assert!(has_entry(&root));
        assert_eq!(count_addons(&root), 2);

        let out = serialize(&root).unwrap();
        let again = parse(&out).unwrap();
        assert!(has_entry(&again));
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Other Tool"));
        assert!(text.contains("-flag"));
        assert!(text.contains(r"C:\popm\popm.exe"));
    }

    #[test]
    fn add_is_idempotent_and_keeps_command_line() {
        let mut root = parse(SAMPLE.as_bytes()).unwrap();
        add_entry(&mut root, "a.exe");
        // give our entry a command line
        for c in root.children.iter_mut() {
            if let Some(e) = c.as_mut_element()
                && e.name == "Launch.Addon" && addon_name(e).as_deref() == Some(ADDON_NAME) {
                    e.children
                        .push(XMLNode::Element(text_element("CommandLine", "--x")));
                }
        }
        add_entry(&mut root, "b.exe");
        assert_eq!(count_addons(&root), 2);
        let text = String::from_utf8(serialize(&root).unwrap()).unwrap();
        assert!(text.contains("b.exe") && !text.contains("a.exe"));
        assert!(text.contains("--x"));
    }

    #[test]
    fn removes_only_our_entry() {
        let mut root = parse(SAMPLE.as_bytes()).unwrap();
        add_entry(&mut root, "a.exe");
        assert!(remove_entry(&mut root));
        assert!(!has_entry(&root));
        assert_eq!(count_addons(&root), 1);
        assert!(!remove_entry(&mut root));
    }

    #[test]
    fn latin1_content_is_accepted() {
        let mut bytes =
            b"<?xml version=\"1.0\" encoding=\"Windows-1252\"?><SimBase.Document><Descr>caf"
                .to_vec();
        bytes.push(0xE9); // é in Latin-1, invalid UTF-8
        bytes.extend_from_slice(b"</Descr></SimBase.Document>");
        let root = parse(&bytes).unwrap();
        assert_eq!(
            root.get_child("Descr").unwrap().get_text().unwrap(),
            "caf\u{e9}"
        );
    }

    #[test]
    fn utf8_bom_is_accepted() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(SAMPLE.as_bytes());
        assert!(parse(&bytes).is_ok());
    }

    #[test]
    fn file_round_trip_creates_backup() {
        let dir = std::env::temp_dir().join(format!("popm-autostart-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("exe.xml");
        std::fs::write(&path, SAMPLE).unwrap();

        activate_at(&path, "x.exe").unwrap();
        assert!(dir.join("exe.xml.backup").exists());
        assert!(has_entry(&parse(&std::fs::read(&path).unwrap()).unwrap()));

        deactivate_at(&path).unwrap();
        assert!(!has_entry(&parse(&std::fs::read(&path).unwrap()).unwrap()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn creates_new_document_when_missing() {
        let dir = std::env::temp_dir().join(format!("popm-autostart-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("exe.xml");
        activate_at(&path, "x.exe").unwrap();
        let root = parse(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(root.name, "SimBase.Document");
        assert!(has_entry(&root));
        std::fs::remove_dir_all(&dir).ok();
    }
}
