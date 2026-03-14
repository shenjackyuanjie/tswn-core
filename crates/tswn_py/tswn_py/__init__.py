"""
Python 包入口。

说明：
- 该文件存在的目的主要是让 setuptools 能稳定发现 `tswn_py` 这个包目录并打进 wheel。
- 实际导出的符号来自同包内的 PyO3 扩展模块：`tswn_py/tswn_py.*`。
"""

from .tswn_py import *  # type: ignore F401,F403