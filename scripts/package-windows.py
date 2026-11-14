"""Author a native, per-user WiX MSI without installer-time scripts."""

import argparse
import base64
import hashlib
import json
import re
import shutil
import uuid
from pathlib import Path
from xml.etree import ElementTree as ET


NAMESPACE = "http://wixtoolset.org/schemas/v4/wxs"
UPGRADE_CODE = "80BE85CF-E630-4ACB-B9D7-E51985D4C2BB"
COMPONENT_NAMESPACE = uuid.UUID("74ce7c15-b2af-42cb-928f-b06c2a71cd14")
ET.register_namespace("", NAMESPACE)


def element(parent, tag, **attributes):
    return ET.SubElement(parent, f"{{{NAMESPACE}}}{tag}", attributes)


def identifier(prefix, value):
    return prefix + hashlib.sha256(value.casefold().encode("utf-8")).hexdigest()[:32]


def author(stage, output, version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("MSI requires a three-part numeric Cargo version")
    if any(int(part) > limit for part, limit in zip(version.split("."), (255, 255, 65535))):
        raise ValueError("Version exceeds Windows Installer limits")
    integration_root = stage / "integrations"
    artifacts = integration_root / "artifacts"
    artifacts.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(stage / "dist/integrations", artifacts, dirs_exist_ok=True)
    shutil.copyfile(stage / "lexington.exe", stage / "lexington-native-host.exe")
    chromium = json.loads((artifacts / "chromium/manifest.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(base64.b64decode(chromium["key"])).hexdigest()[:32]
    extension_id = "".join(chr(ord("a") + int(digit, 16)) for digit in digest)
    common = dict(name="com.lexington.context", description="Lexington context menu bridge",
                  path=r"..\lexington-native-host.exe", type="stdio")
    for browser, permissions in (
        ("firefox", dict(allowed_extensions=["lexington-context@lexington.local"])),
        ("chromium", dict(allowed_origins=[f"chrome-extension://{extension_id}/"])),
    ):
        (integration_root / f"native-{browser}.json").write_text(
            json.dumps(dict(common, **permissions), indent=2) + "\n", encoding="utf-8")

    wix = ET.Element(f"{{{NAMESPACE}}}Wix")
    package = element(wix, "Package", Name="Lexington", Manufacturer="Lexington",
                      Version=version, UpgradeCode=UPGRADE_CODE, Scope="perUser",
                      Language="1033", Codepage="65001", InstallerVersion="500")
    element(package, "SummaryInformation", Description="Lexington native desktop installer")
    element(package, "MajorUpgrade", AllowSameVersionUpgrades="yes", Schedule="afterInstallInitialize",
            DowngradeErrorMessage="A newer version of Lexington is already installed.")
    element(package, "MediaTemplate", EmbedCab="yes", CompressionLevel="high")
    element(package, "Property", Id="RESTARTMANAGERCONTROL", Value="DisableShutdown")
    element(package, "Property", Id="ARPNOMODIFY", Value="1")
    saved_directory = element(package, "Property", Id="INSTALLFOLDER", Secure="yes")
    element(saved_directory, "RegistrySearch", Id="SavedInstallDirectory", Root="HKCU",
            Key=r"Software\Lexington\Installer", Name="InstallDir", Type="raw", Bitness="always64")
    local = element(package, "StandardDirectory", Id="LocalAppDataFolder")
    install = element(local, "Directory", Id="INSTALLFOLDER", Name="lexington")
    element(package, "StandardDirectory", Id="DesktopFolder")
    element(package, "StandardDirectory", Id="ProgramMenuFolder")
    feature = element(package, "Feature", Id="Main", Title="Lexington", Level="1")
    directories = {"": install}

    def directory(relative):
        key = relative.as_posix() if relative.parts else ""
        if key not in directories:
            directories[key] = element(directory(relative.parent), "Directory",
                                       Id=identifier("dir_", key), Name=relative.name)
        return directories[key]

    def component(parent, key):
        component_id = identifier("cmp_", key)
        node = element(parent, "Component", Id=component_id,
                       Guid=str(uuid.uuid5(COMPONENT_NAMESPACE, key.casefold())), Bitness="always64")
        element(node, "RegistryValue", Root="HKCU", Key=r"Software\Lexington\Installer\Components",
                Name=component_id, Type="integer", Value="1", KeyPath="yes")
        element(feature, "ComponentRef", Id=component_id)
        return node

    payload = [stage / "lexington.exe", stage / "lexington-native-host.exe", stage / "README.md"]
    for folder in (stage / "resource", integration_root):
        payload.extend(path for path in sorted(folder.rglob("*")) if path.is_file()
                       and "live2d-drafts" not in path.relative_to(stage).parts
                       and not any(part.startswith("linux-") for part in path.relative_to(stage).parts))
    seen = set()
    for source in payload:
        relative = source.relative_to(stage)
        key = relative.as_posix()
        if key.casefold() in seen:
            raise ValueError(f"Case-insensitive payload collision: {key}")
        seen.add(key.casefold())
        node = component(directory(relative.parent), key)
        element(node, "File", Id=identifier("file_", key), Source=str(source), Name=source.name)

    node = component(install, "installer-integration")
    element(node, "RegistryValue", Root="HKCU", Key=r"Software\Lexington\Installer",
            Name="Version", Type="string", Value=version)
    element(node, "RegistryValue", Root="HKCU", Key=r"Software\Lexington\Installer",
            Name="InstallDir", Type="string", Value="[INSTALLFOLDER]")
    for browser in (r"Google\Chrome", "Chromium", r"Microsoft\Edge", r"BraveSoftware\Brave-Browser", "Mozilla"):
        manifest = "firefox" if browser == "Mozilla" else "chromium"
        element(node, "RegistryValue", Root="HKCU",
                Key=rf"Software\{browser}\NativeMessagingHosts\com.lexington.context",
                Type="string", Value=rf"[INSTALLFOLDER]integrations\native-{manifest}.json")
    for kind in ("*", "Directory"):
        key = rf"Software\Classes\{kind}\shell\Lexington"
        element(node, "RegistryValue", Root="HKCU", Key=key,
                Type="string", Value="看看Lexington怎么说")
        element(node, "RegistryValue", Root="HKCU", Key=key, Name="Icon",
                Type="string", Value='"[INSTALLFOLDER]lexington.exe"')
        element(node, "RegistryValue", Root="HKCU", Key=key + r"\command", Type="string",
                Value='"[INSTALLFOLDER]lexington.exe" --ask-files "%1"')
    for folder in ("DesktopFolder", "ProgramMenuFolder"):
        element(node, "Shortcut", Id="shortcut_" + folder, Directory=folder, Name="Lexington",
                Target="[INSTALLFOLDER]lexington.exe", WorkingDirectory="INSTALLFOLDER",
                Icon="LexingtonIcon", IconIndex="0")
    icon = stage / "resource/icon/Normal.ico"
    if not icon.is_file():
        raise ValueError("Missing application icon: resource/icon/Normal.ico")
    element(package, "Icon", Id="LexingtonIcon", SourceFile=str(icon))
    element(package, "Property", Id="ARPPRODUCTICON", Value="LexingtonIcon")
    for key, folder in directories.items():
        cleanup = component(folder, "cleanup/" + key)
        element(cleanup, "RemoveFolder", Id=identifier("remove_", key), On="uninstall")
    ET.indent(wix)
    output.write_bytes(ET.tostring(wix, encoding="utf-8", xml_declaration=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, required=True)
    arguments = parser.parse_args()
    cargo = arguments.cargo.read_text(encoding="utf-8")
    package = cargo.split("[package]", 1)[1].split("\n[", 1)[0]
    version = re.search(r'^version\s*=\s*"([^"]+)"', package, re.MULTILINE)
    if version is None:
        raise ValueError("Missing Cargo package version")
    author(arguments.stage.resolve(), arguments.output.resolve(), version.group(1))


if __name__ == "__main__":
    main()
