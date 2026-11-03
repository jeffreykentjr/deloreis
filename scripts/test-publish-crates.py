#!/usr/bin/env python3
"""Checks for release selection and safe resumption, without a registry token."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("publisher", Path(__file__).with_name("publish-crates.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


def metadata():
    packages = [{"id": name, "name": name, "version": "0.1.0", "publish": ["crates-io"]}
                for name in sorted(publisher.PUBLISHED_CRATES)]
    packages.append({"id": "xtask", "name": "xtask", "version": "0.1.0", "publish": []})
    return {"packages": packages, "workspace_members": [package["id"] for package in packages]}


class PublicationChecks(unittest.TestCase):
    def test_release_set_and_version(self):
        packages, version = publisher.select_packages(metadata(), "0.1.0")
        self.assertEqual({package["name"] for package in packages}, publisher.PUBLISHED_CRATES)
        self.assertEqual(version, "0.1.0")
        with self.assertRaises(ValueError):
            publisher.select_packages(metadata(), "0.2.0")

    def test_refuses_missing_or_unapproved_package(self):
        missing = metadata()
        missing["packages"][0]["publish"] = []
        with self.assertRaises(ValueError):
            publisher.select_packages(missing)
        extra = metadata()
        extra["packages"].append({"id": "dli-extra", "name": "dli-extra", "version": "0.1.0", "publish": ["crates-io"]})
        extra["workspace_members"].append("dli-extra")
        with self.assertRaises(ValueError):
            publisher.select_packages(extra)

    def test_resume_requires_identical_unyanked_bytes(self):
        package = {"name": "deloreis", "version": "0.1.0"}
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "deloreis-0.1.0.crate"
            archive.write_bytes(b"checked package")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with patch.object(publisher.time, "sleep"), patch.object(publisher, "registry_version", return_value=None):
                self.assertEqual(publisher.pending_packages([package], Path(directory)), [package])
            for entry in [
                {"name": "deloreis", "cksum": "wrong", "yanked": False},
                {"name": "deloreis", "cksum": digest, "yanked": True},
            ]:
                with patch.object(publisher.time, "sleep"), patch.object(publisher, "registry_version", return_value=entry):
                    with self.assertRaises(ValueError):
                        publisher.pending_packages([package], Path(directory))
            with patch.object(publisher.time, "sleep"), patch.object(publisher, "registry_version", return_value={"name": "deloreis", "cksum": digest, "yanked": False}):
                self.assertEqual(publisher.pending_packages([package], Path(directory)), [])

    def test_packaged_resources_match(self):
        publisher.check_resources()


if __name__ == "__main__":
    unittest.main()
