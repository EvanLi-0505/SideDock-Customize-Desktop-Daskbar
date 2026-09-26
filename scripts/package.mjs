// Packages the release build as a portable folder + zip:
//
//   release/SideDock/SideDock.exe                 (runnable in place)
//   release/SideDock/LICENSE
//   release/SideDock_<version>_x64_portable.zip
//
// No installer on purpose: SideDock runs from wherever the user unzips it and keeps
// all of its data in a `data` folder next to the executable.
//
// `release/SideDock/data` (created when the build is run in place) is preserved, and
// the zip is built from a clean staging folder so it never contains personal data.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

const exe = join(root, "target", "release", "SideDock.exe");
if (!existsSync(exe)) {
  console.error(`missing ${exe}, run "npm run build" first`);
  process.exit(1);
}

const out = join(root, "release");
const folder = join(out, "SideDock");
const staging = join(out, ".staging", "SideDock");
const zip = join(out, `SideDock_${version}_x64_portable.zip`);

function copyApp(target) {
  mkdirSync(target, { recursive: true });
  copyFileSync(exe, join(target, "SideDock.exe"));
  copyFileSync(join(root, "LICENSE"), join(target, "LICENSE"));
}

try {
  copyApp(folder);
} catch (err) {
  console.error(`cannot update ${folder}: is SideDock running from there? Quit it first.\n${err}`);
  process.exit(1);
}

rmSync(join(out, ".staging"), { recursive: true, force: true });
rmSync(zip, { force: true });
copyApp(staging);
execFileSync(
  "powershell.exe",
  ["-NoProfile", "-Command", `Compress-Archive -Path '${staging}' -DestinationPath '${zip}' -Force`],
  { stdio: "inherit" },
);
rmSync(join(out, ".staging"), { recursive: true, force: true });

const mb = (p) => (statSync(p).size / 1024 / 1024).toFixed(1);
console.log(`\nSideDock ${version}`);
console.log(`  exe: ${join(folder, "SideDock.exe")} (${mb(join(folder, "SideDock.exe"))} MB)`);
console.log(`  zip: ${zip} (${mb(zip)} MB)`);
