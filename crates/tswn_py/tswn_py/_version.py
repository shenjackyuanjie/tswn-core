"""
运行时从同级 Cargo.toml 读取版本号。

说明
----
* ``Cargo.toml`` 是版本的唯一来源；``pyproject.toml`` 通过
  ``[tool.setuptools.dynamic] version = {attr = "tswn_py._version.__version__"}``
  在构建时调用本模块获取版本字符串。
* 本模块只使用标准库（``tomllib`` Python 3.11+ 内置），无额外依赖。
* 安装后，``Cargo.toml`` 已不再随包分发，此时直接回退到
  ``importlib.metadata`` 从已安装的包元数据中读取版本。
"""

from __future__ import annotations

import tomllib
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path


def _read_version() -> str:
    # 优先从 Cargo.toml 读取（源码树 / 构建期）
    cargo_toml = Path(__file__).parent.parent / "Cargo.toml"
    if cargo_toml.is_file():
        with cargo_toml.open("rb") as f:
            data = tomllib.load(f)
        return data["package"]["version"]

    # 回退：已安装的包元数据（wheel 安装后 Cargo.toml 不再存在）
    try:
        return version("tswn_py")
    except PackageNotFoundError:
        return "0.0.0.dev0"


__version__: str = _read_version()