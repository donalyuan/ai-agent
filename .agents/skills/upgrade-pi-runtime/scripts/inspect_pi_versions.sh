#!/usr/bin/env bash
set -euo pipefail

readonly PI_PACKAGES=(
  "@earendil-works/pi-agent-core"
  "@earendil-works/pi-ai"
  "@earendil-works/pi-storage-sqlite-node"
)
readonly PI_REPOSITORY_URL="https://github.com/earendil-works/pi.git"

offline=false
runtime_dir=""
target_version=""

usage() {
  cat <<'EOF'
用法：inspect_pi_versions.sh [--offline] [--runtime-dir PATH] [--target VERSION]

只读检查 Novex Agent Runtime 的 Pi 版本：
  --offline           仅校验 package.json 与 package-lock.json
  --runtime-dir PATH  指定 Runtime 目录，默认使用 services/agent-runtime
  --target VERSION    验证指定的稳定 npm 版本，而不是仅使用 latest
  -h, --help          显示帮助
EOF
}

while (($# > 0)); do
  case "$1" in
    --offline)
      offline=true
      shift
      ;;
    --runtime-dir)
      if (($# < 2)); then
        printf '错误：--runtime-dir 缺少路径\n' >&2
        exit 2
      fi
      runtime_dir="$2"
      shift 2
      ;;
    --target)
      if (($# < 2)); then
        printf '错误：--target 缺少版本号\n' >&2
        exit 2
      fi
      target_version="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf '错误：未知参数 %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v node >/dev/null 2>&1; then
  printf '错误：缺少 node，无法解析 JSON 清单\n' >&2
  exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ -z "$runtime_dir" ]]; then
  if ! repository_root="$(git -C "$script_dir" rev-parse --show-toplevel 2>/dev/null)"; then
    printf '错误：无法从 Skill 目录定位 Git 仓库根目录\n' >&2
    exit 2
  fi
  runtime_dir="$repository_root/services/agent-runtime"
fi

runtime_dir="$(cd -- "$runtime_dir" 2>/dev/null && pwd)" || {
  printf '错误：Runtime 目录不存在：%s\n' "$runtime_dir" >&2
  exit 2
}
manifest_path="$runtime_dir/package.json"
lockfile_path="$runtime_dir/package-lock.json"

for required_file in "$manifest_path" "$lockfile_path"; do
  if [[ ! -f "$required_file" ]]; then
    printf '错误：缺少文件：%s\n' "$required_file" >&2
    exit 2
  fi
done

local_report="$({ node - "$manifest_path" "$lockfile_path" <<'NODE'
const fs = require("node:fs");

const [manifestPath, lockfilePath] = process.argv.slice(2);
const packages = [
  "@earendil-works/pi-agent-core",
  "@earendil-works/pi-ai",
  "@earendil-works/pi-storage-sqlite-node",
];

function parseJson(path) {
  try {
    return JSON.parse(fs.readFileSync(path, "utf8"));
  } catch (error) {
    console.error(`错误：无法解析 ${path}：${error.message}`);
    process.exit(2);
  }
}

const manifest = parseJson(manifestPath);
const lockfile = parseJson(lockfilePath);
for (const name of packages) {
  const declared = manifest.dependencies?.[name] ?? "MISSING";
  const lockSpecifier = lockfile.packages?.[""]?.dependencies?.[name] ?? "MISSING";
  const resolved = lockfile.packages?.[`node_modules/${name}`]?.version
    ?? lockfile.dependencies?.[name]?.version
    ?? "MISSING";
  process.stdout.write(`${name}\t${declared}\t${lockSpecifier}\t${resolved}\n`);
}
NODE
  } 2>&1)" || {
  printf '%s\n' "$local_report" >&2
  exit 2
}

declare -A declared_versions=()
declare -A resolved_versions=()
local_failure=0
common_current=""

printf 'Runtime：%s\n\n' "$runtime_dir"
printf '%-48s %-12s %-12s %-12s\n' '包' 'manifest' 'lock spec' 'resolved'
while IFS=$'\t' read -r package_name declared lock_spec resolved; do
  declared_versions["$package_name"]="$declared"
  resolved_versions["$package_name"]="$resolved"
  printf '%-48s %-12s %-12s %-12s\n' "$package_name" "$declared" "$lock_spec" "$resolved"

  if [[ ! "$declared" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    printf '错误：%s 未使用稳定版精确版本：%s\n' "$package_name" "$declared" >&2
    local_failure=1
  fi
  if [[ "$declared" != "$lock_spec" || "$declared" != "$resolved" ]]; then
    printf '错误：%s 的 manifest、lock spec 与 resolved 版本不一致\n' "$package_name" >&2
    local_failure=1
  fi
  if [[ -z "$common_current" ]]; then
    common_current="$declared"
  elif [[ "$declared" != "$common_current" ]]; then
    printf '错误：三个 Pi 包未锁定到同一版本\n' >&2
    local_failure=1
  fi
done <<<"$local_report"

if ((local_failure != 0)); then
  exit 1
fi

printf '\n本地版本：%s（三包一致）\n' "$common_current"
if [[ "$offline" == true ]]; then
  printf '上游检查：已跳过（--offline）\n'
  exit 0
fi

if ! command -v npm >/dev/null 2>&1 || ! command -v git >/dev/null 2>&1; then
  printf '错误：联网检查需要 npm 和 git\n' >&2
  exit 2
fi
if [[ -n "$target_version" && ! "$target_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf '错误：--target 只接受稳定版精确版本（例如 0.82.0）\n' >&2
  exit 2
fi

inspection_tmp="$(mktemp -d)"
trap 'rm -rf -- "$inspection_tmp"' EXIT
export NPM_CONFIG_CACHE="$inspection_tmp/npm-cache"
export NPM_CONFIG_UPDATE_NOTIFIER=false

declare -A latest_versions=()
upstream_failure=0
common_latest=""

printf '\n%-48s %-12s\n' '包' 'npm latest'
for package_name in "${PI_PACKAGES[@]}"; do
  if latest="$(npm --silent view "$package_name" version 2>/dev/null)" \
    && [[ "$latest" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    latest_versions["$package_name"]="$latest"
    printf '%-48s %-12s\n' "$package_name" "$latest"
    if [[ -z "$common_latest" ]]; then
      common_latest="$latest"
    elif [[ "$latest" != "$common_latest" ]]; then
      printf '错误：三个 Pi 包的 npm latest 不一致\n' >&2
      upstream_failure=1
    fi
  else
    printf '%-48s %-12s\n' "$package_name" '查询失败'
    printf '错误：无法确认 %s 的 npm latest\n' "$package_name" >&2
    upstream_failure=1
  fi
done

if ! remote_tags="$(git ls-remote --tags --refs "$PI_REPOSITORY_URL" 2>/dev/null)"; then
  printf '错误：无法查询 Pi GitHub tags\n' >&2
  upstream_failure=1
  remote_tags=""
fi

github_latest="$(printf '%s\n' "$remote_tags" \
  | awk -F/ '/refs\/tags\/v[0-9]+\.[0-9]+\.[0-9]+$/ { print $3 }' \
  | sed 's/^v//' \
  | sort -V \
  | tail -n 1)"
if [[ -z "$github_latest" ]]; then
  printf '错误：未找到 GitHub 稳定版本 tag\n' >&2
  upstream_failure=1
else
  printf '\nGitHub 最新稳定 tag：v%s\n' "$github_latest"
fi

candidate_version="${target_version:-$common_latest}"
if [[ -z "$candidate_version" ]]; then
  printf '错误：无法确定候选版本\n' >&2
  upstream_failure=1
else
  printf '候选版本：%s%s\n' "$candidate_version" "$([[ -n "$target_version" ]] && printf '（显式目标）' || printf '（共同 npm latest）')"

  if [[ -n "$target_version" ]]; then
    for package_name in "${PI_PACKAGES[@]}"; do
      if published="$(npm --silent view "${package_name}@${target_version}" version 2>/dev/null)" \
        && [[ "$published" == "$target_version" ]]; then
        printf '目标发布：%s@%s 已存在\n' "$package_name" "$target_version"
      else
        printf '错误：目标发布缺失：%s@%s\n' "$package_name" "$target_version" >&2
        upstream_failure=1
      fi
    done
  fi

  if ! awk -v expected="refs/tags/v${candidate_version}" '$2 == expected { found = 1 } END { exit !found }' <<<"$remote_tags"; then
    printf '错误：GitHub 缺少对应 tag v%s\n' "$candidate_version" >&2
    upstream_failure=1
  fi

  highest_version="$(printf '%s\n%s\n' "$common_current" "$candidate_version" | sort -V | tail -n 1)"
  if [[ "$candidate_version" == "$common_current" ]]; then
    printf '结论：当前已是候选版本。\n'
  elif [[ "$highest_version" == "$candidate_version" ]]; then
    printf '结论：发现可升级版本 %s -> %s；本次检查未修改项目。\n' "$common_current" "$candidate_version"
  else
    printf '错误：目标版本 %s 低于当前版本 %s，升级流程不接受降级\n' "$candidate_version" "$common_current" >&2
    upstream_failure=1
  fi
fi

if ((upstream_failure != 0)); then
  printf '结论：上游信息不完整或不一致，不足以进入升级。\n' >&2
  exit 1
fi
