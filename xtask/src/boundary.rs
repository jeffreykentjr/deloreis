//! Repository-wide guard against retired private-world names.

use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest as _, Sha256};

/// Digests of retired tokens and their accepted separator variants.
const TOKEN_DIGESTS: &[&str] = &[
    "0115e19e1b9249cd7ce52773aeb19e3358a515ad1df1ab1bb7fdfdf41ddd096f",
    "042ba62725a66a3c48bb563157ebdd61498a7934853f61cea186bc426f3eb0dc",
    "0437b7605a3fb94f1b4fd0b19c4529ce951262adc08d3c98132366546d1c58fd",
    "115c12975e8856e59594ac1cdeed811fe66a35518a930a66e499ace51dd16d4f",
    "132a25515836d4d4501ffd71a0a699eac273bbd23d12fd3f5dd66e7405de65de",
    "2ab62cfe39e1ff0045c106d6ed1497bf4b6742edb85b85e08583067bc0e865e3",
    "2b4179ddaa3e5bf675f92768baee86371be184168b5541ffd7a02013fa619bcf",
    "2c0e1a455dbb9a94bb0f6072b3781fa2b9a409c0c529ea67a9adeacc223bbdaa",
    "2e6265247e89b12d6ec6a78251197d103b78eef4b8575d0c24c5f13b2508e057",
    "3bc32f89505a9f352ff3aca0c51f3780518d0b09ac7076ecca1ed86f52d24196",
    "3dc47a3bf7667f5a31cf447803ab7accc0108d7b9263fe8957ed2e4f03d09de5",
    "3f2d45d86839165a135cb00fe4869134db1fc5a54615815feb0d2e953808ed63",
    "3fd8504edcbf2aa9e7886055e0697bedc4cc67740f39434df46acdce57ad572a",
    "4307e402589de663896c69b4c04f86b048f60d0cfcd7d768c40d97632998c526",
    "4454a66edc50d32e99a51f82a2276cc9974f124e61b59bad591b6bc7ed91bf0e",
    "48f4bccf866890d326b6a7be44df5ce5e31e418173e7f73bed2b3bb236680dd2",
    "4f8f44b4e2eb484e0f0d08aa378d387c829e910e11ab08acb9653e7a3e2a7c7c",
    "52a774bed34f1b791e811ef44ffb02d180bc55c52e662d06faa3be125798b2bb",
    "52cb04deb48ae626d110f8870eac2536b7bd94f76ab462b3c84688ea2c9b6621",
    "623ab33609ebf4183b607099eedfe56a52e38f1627ff085b3becc793f8d11757",
    "634af93d45446e6ab15df11aafd18e710968e41e7a7f486825ff8f19b83bef98",
    "6d34571e9322e0918e9ba49660afd39d1fd4ce7e89e3b9f05ada942ce60682e8",
    "71ce290c2541f3df44f2b6391737a7febe816e1b4614048c955866d2f45debf7",
    "7d88648e114cc7fe0762454ae270de78e83a7bec3c4a1534190cdb274345e359",
    "7f010cb22e82cfce4b51570f53f6ecd2dd78e569ff7bee70d00abfd8f30ab908",
    "85fde724394e1fcb326c05a0c7e392760f3ec2d2e029f0fed5b2e61df8fbf21d",
    "94a21a8288ff75211c14a2e5bb164fe4d547a7ce67f4c3bd45a8659ef3039fc8",
    "9d6bf1e72a27c2299e19c8cd047ba14334bd256fce06d64c639d06cb3cd49f1b",
    "a05253f95b82d61fb3be3554decc26dcd18f34f3d5ece092ea441eda149dbcc6",
    "a344087330efe5dfbf18a7bf4c2eef22371b49b5551bd58e703a9ff47333dccf",
    "a3cc34c68168ef40dcd57430dc47618b0a896214c048f97c55815b0be5a01b9b",
    "a5377606a1be1613ef16f6f3bcef4714fd1590d7de61d7956548bf99a7900cca",
    "a60440656d76505aa82d735f1e63091dd4c0192521021db85d31bca288b2bf7d",
    "a901a01ff22e1d84e397961dc9e597f2be608d473f3dd2a41433ee1270756835",
    "aae8a420e664b4dc04cb882164d9d020af9f7b45e9eb10505877338606c34670",
    "add9449204689e24fa629774dc2de6204ae340d144e500d174370ade9b4af807",
    "ae43e66e5ca540464f9c30997f18a5c90ff2ec988c6aa4ea148a11e49035adfc",
    "ae62a6a8d07e02b1c07896f2f2987764b2f2925a1f0b01f803ddef03f7ac07a8",
    "b20b4f0ac60b3843602def33842457369e656a68d9e1d264190cbedb3eda673b",
    "b69cd406d8411bb993c9d31c730377299c44845cd064e1543f2115ccc6d07ff5",
    "b7c75032c320bb020d04f8837de76e62edc3ac9c869d2c02f89aa3035c411283",
    "bf8c10b359f1143758b4f82d07eef1bcd63f67d9fc6eb48146ce1f488aef8ed5",
    "c35ce2c703fe8d2fd7493c13f6767f8ef3ed83bbf614902f8af3a0e1310676fa",
    "c4e572f5cf09b51e666f5675ed532963b68e48c0f517239ee75c96f7453cde2c",
    "c8b6c061e8591dfeaabefdbad7cdacc0ece53900f0bb02160860a0e3ef7d2acc",
    "d243171b70d9b54a67f8060c1f2a395bf077e54735af81982d3a5a3ddb8afd85",
    "d29679716a1f3d5777abe6ec2212bce58f6af207b6f3c51f1ff660e24316cc11",
    "d47566c6c1a9fda8223eaa5a5947e29fd8d40473c2c115f896ca2284f9c6d9c7",
    "d4b0f2ac272f3d30a93a12ac7d55f5c385b9cba0aa147ad84fa526ce35d3cd65",
    "de4e6ab91f48332b2e8a65e333a8e7bac7fe9833a0683aa298c837763264f9b5",
    "e4b02fb3d8b539ccce6125a8d5c36d5dfce9dcc0876a108c6c5c3c63d1aa42ab",
    "e95cb17f6da89acaac90bb8b15fb13b05a8fb40b6ed07b5436b04fdc8cc34cbd",
    "eab1a513b419632272007104c3174a9b86a7fe538f2a0b27b41e919b504593dc",
    "ede80ca3f32502513bceabfa936c83eb580625caa3276b5ba2e9e52ada37c334",
    "f30fc0e061eba665e0a964fdcd418a723c5c940bb576cbcd41d3659b852dc8cf",
    "f7ef4a714de37b8e600a7b50e66ddeae61b174e0fb3a086cfd3e71b1f4349ee2",
    "f96edd044e8d7685152f42209da0e8170493c4cf2429a06f7d31dae21f4c94b5",
    "fa333efd4003e98d0950bd8b977e34c0de50ae12fa17e9cbf8355ad5c3774e06",
];

const TOKEN_LENGTHS: &[usize] = &[3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 17, 19];
const INFLECTED_PREFIX_DIGEST: &str =
    "94a21a8288ff75211c14a2e5bb164fe4d547a7ce67f4c3bd45a8659ef3039fc8";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Match whole tokens at ASCII boundaries, including the registered inflection.
fn find_match(
    text: &str,
    digests: &HashSet<&str>,
    lengths: &[usize],
    inflected: &str,
) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let maximum = lengths.iter().copied().max().unwrap_or(0);
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_alphanumeric()
            || start
                .checked_sub(1)
                .is_some_and(|index| bytes[index].is_ascii_alphanumeric())
        {
            continue;
        }
        for end in start + 1..=(start + maximum).min(bytes.len()) {
            let byte = bytes[end - 1];
            if !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_' | b' ' | b'.') {
                break;
            }
            if !byte.is_ascii_alphanumeric() || !lengths.contains(&(end - start)) {
                continue;
            }
            let boundary = bytes
                .get(end)
                .is_none_or(|next| !next.is_ascii_alphanumeric());
            let inflection = end - start == 6 && bytes.get(end..end + 2) == Some(b"ed");
            if boundary || inflection {
                let candidate = digest(&bytes[start..end]);
                if (boundary && digests.contains(candidate.as_str()))
                    || (inflection && candidate == inflected)
                {
                    return Some(start);
                }
            }
        }
    }
    None
}

fn scan_file(
    path: &Path,
    label: &str,
    digests: &HashSet<&str>,
    hits: &mut Vec<String>,
) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {label}"))?;
    let text = String::from_utf8_lossy(&bytes);
    if let Some(offset) = find_match(&text, digests, TOKEN_LENGTHS, INFLECTED_PREFIX_DIGEST) {
        let line = text[..offset].matches('\n').count() + 1;
        hits.push(format!("{label}:{line}: matching content"));
    }
    Ok(())
}

/// Check all tracked files and every file selected for a publishable package.
pub fn run_boundary(root: &Path) -> Result<()> {
    let root = root.canonicalize().context("resolving workspace root")?;
    let digests: HashSet<&str> = TOKEN_DIGESTS.iter().copied().collect();
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .context("listing tracked files for boundary guard")?;
    if !output.status.success() {
        bail!("boundary: git could not list tracked files");
    }
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();
    for name in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        let name = std::str::from_utf8(name).context("non-UTF-8 tracked path")?;
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        if find_match(name, &digests, TOKEN_LENGTHS, INFLECTED_PREFIX_DIGEST).is_some() {
            hits.push(format!("{name}: matching path"));
        }
        scan_file(&path, name, &digests, &mut hits)?;
        seen.insert(path.canonicalize()?);
    }
    scan_packages(&root, &digests, &mut seen, &mut hits)?;
    if !hits.is_empty() {
        bail!(
            "boundary: retired world content found:\n{}",
            hits.join("\n")
        );
    }
    println!("boundary: tracked files and package contents are clear");
    Ok(())
}

fn scan_packages(
    root: &Path,
    digests: &HashSet<&str>,
    seen: &mut BTreeSet<PathBuf>,
    hits: &mut Vec<String>,
) -> Result<()> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--locked",
            "--format-version",
            "1",
        ])
        .current_dir(root)
        .output()
        .context("reading workspace package metadata")?;
    if !output.status.success() {
        bail!(
            "boundary: cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let members = metadata["workspace_members"]
        .as_array()
        .context("missing workspace members")?;
    let packages = metadata["packages"]
        .as_array()
        .context("missing packages")?;
    for package in packages {
        if !members.contains(&package["id"])
            || package["publish"].as_array().is_some_and(Vec::is_empty)
        {
            continue;
        }
        let name = package["name"].as_str().context("package has no name")?;
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .context("package has no manifest")?,
        );
        let crate_root = manifest.parent().context("manifest has no parent")?;
        // Listing dirty files is intentional: the guard checks exactly what
        // Cargo would include, even before changes are committed. Nothing is published.
        let output = Command::new("cargo")
            .args([
                "package",
                "--list",
                "--allow-dirty",
                "--offline",
                "--locked",
                "--manifest-path",
            ])
            .arg(manifest)
            .current_dir(root)
            .output()
            .with_context(|| format!("listing package {name}"))?;
        if !output.status.success() {
            bail!(
                "boundary: cargo package --list failed for {name}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let listing = std::str::from_utf8(&output.stdout).context("non-UTF-8 package listing")?;
        for entry in listing.lines() {
            let relative = Path::new(entry);
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                bail!("boundary: unsafe path in package {name}");
            }
            let label = format!("{name}/{entry}");
            if find_match(&label, digests, TOKEN_LENGTHS, INFLECTED_PREFIX_DIGEST).is_some() {
                hits.push(format!("{label}: matching package path"));
            }
            // Cargo generates these files; their inputs are checked separately.
            if entry == ".cargo_vcs_info.json" {
                continue;
            }
            let path = if entry == "Cargo.toml.orig" {
                manifest.to_owned()
            } else if entry == "Cargo.lock" && !crate_root.join(entry).exists() {
                root.join("Cargo.lock")
            } else if entry == "README.md" && !crate_root.join(entry).exists() {
                crate_root.join(
                    package["readme"]
                        .as_str()
                        .context("missing package readme")?,
                )
            } else {
                crate_root.join(entry)
            };
            let canonical = path
                .canonicalize()
                .with_context(|| format!("resolving {label}"))?;
            if !canonical.starts_with(root) {
                bail!("boundary: {label} is outside the workspace");
            }
            if seen.insert(canonical.clone()) {
                scan_file(&canonical, &label, digests, hits)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{digest, find_match, scan_packages};
    use std::collections::{BTreeSet, HashSet};
    use std::process::Command;

    #[test]
    fn token_boundaries_are_case_insensitive_and_not_substrings() {
        let hashes: Vec<String> = ["blue-bell", "blue bell", "blue_bell"]
            .iter()
            .map(|v| digest(v.as_bytes()))
            .collect();
        let set: HashSet<&str> = hashes.iter().map(String::as_str).collect();
        let check = |text| find_match(text, &set, &[9], "").is_some();
        assert!(check("A BLUE-BELL rings"));
        assert!(!check("blue-bellflower"));
        assert!(!check("notblue-bell"));
        assert!(check("blue-bell's song"));
        assert!(check("a Blue Bell rings"));
        assert!(check("blue_bell rings"));
        assert!(!check("blue belling"));
        let prefix = digest(b"branch");
        assert!(find_match("a branched path", &HashSet::new(), &[6], &prefix).is_some());
        assert!(find_match("a BRANCHEDNESS path", &HashSet::new(), &[6], &prefix).is_some());
        assert!(find_match("a branching path", &HashSet::new(), &[6], &prefix).is_none());
    }

    #[test]
    fn package_scan_checks_untracked_contents_and_paths() {
        let dir = tempfile::tempdir().expect("temporary workspace");
        let root = dir.path().canonicalize().expect("workspace path");
        std::fs::create_dir(root.join("src")).expect("source directory");
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"boundary-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");
        std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("source");
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .current_dir(&root)
                .status()
                .expect("git init")
                .success()
        );
        assert!(
            Command::new("git")
                .args(["add", "--", "Cargo.toml", "src/lib.rs"])
                .current_dir(&root)
                .status()
                .expect("git add")
                .success()
        );
        std::fs::write(root.join("src/note.txt"), "a BLUE-BELL rings\n").expect("untracked file");
        std::fs::write(root.join("src/blue-bell.txt"), "ordinary text\n").expect("untracked path");
        assert!(
            Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&root)
                .status()
                .expect("lockfile")
                .success()
        );
        let hash = digest(b"blue-bell");
        let digests: HashSet<&str> = [hash.as_str()].into_iter().collect();
        let mut seen = BTreeSet::new();
        let mut hits = Vec::new();
        scan_packages(&root, &digests, &mut seen, &mut hits).expect("scan packages");
        assert!(
            hits.iter()
                .any(|hit| hit.contains("src/note.txt:1: matching content")),
            "{hits:?}"
        );
        assert!(
            hits.iter()
                .any(|hit| hit.contains("src/blue-bell.txt: matching package path")),
            "{hits:?}"
        );
    }
}
