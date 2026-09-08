"""Compare actual native Rust CLI, Python extension, C DLL and WASM JSON payloads."""
import ctypes as C
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'target/py_cli_api_verify/import'))
import tswn_py


class Options(C.Structure):
    _fields_ = [('struct_size', C.c_uint32), ('eval_rq', C.c_double),
                ('max_rounds', C.c_size_t), ('include_icons', C.c_uint8)]


class String(C.Structure):
    _fields_ = [('ptr', C.c_void_p), ('len', C.c_size_t)]


def main():
    library_name = 'tswn_capi.dll' if os.name == 'nt' else 'libtswn_capi.so'
    lib = C.CDLL(str(ROOT / 'target/debug' / library_name))
    lib.tswn_battle_options_default.argtypes = [C.POINTER(Options)]
    lib.tswn_battle_options_default.restype = None
    lib.tswn_battle_session_new.argtypes = [C.c_char_p, C.POINTER(Options), C.POINTER(C.c_void_p)]
    lib.tswn_battle_session_free.argtypes = [C.c_void_p]
    lib.tswn_battle_session_free.restype = None
    lib.tswn_str_free.argtypes = [String]
    lib.tswn_str_free.restype = None
    for name in ['initial_states_json', 'current_states_json']:
        getattr(lib, 'tswn_battle_session_' + name).argtypes = [C.c_void_p, C.POINTER(String)]
    for name in ['next_frame_json', 'result_json']:
        getattr(lib, 'tswn_battle_session_' + name).argtypes = [C.c_void_p, C.POINTER(C.c_uint8), C.POINTER(String)]
    for name in ['status', 'stop_reason']:
        getattr(lib, 'tswn_battle_session_' + name).argtypes = [C.c_void_p, C.POINTER(C.c_uint32)]

    def c_battle(raw, max_rounds):
        options = Options()
        lib.tswn_battle_options_default(C.byref(options))
        options.max_rounds = max_rounds
        handle = C.c_void_p()
        assert lib.tswn_battle_session_new(raw.encode(), C.byref(options), C.byref(handle)) == 0

        def get(name, optional=False):
            text, has = String(), C.c_uint8()
            args = [handle, C.byref(has), C.byref(text)] if optional else [handle, C.byref(text)]
            assert getattr(lib, 'tswn_battle_session_' + name)(*args) == 0
            try:
                if optional and not has.value:
                    assert not text.ptr and text.len == 0
                    return None
                return json.loads(C.string_at(text.ptr, text.len))
            finally:
                lib.tswn_str_free(text)
        try:
            initial, frames = get('initial_states_json'), []
            assert get('result_json', True) is None
            while (frame := get('next_frame_json', True)) is not None:
                frames.append(frame)
            result = get('result_json', True)
            assert get('current_states_json') == result['final_states']
            status, reason = C.c_uint32(), C.c_uint32()
            assert lib.tswn_battle_session_status(handle, C.byref(status)) == 0
            assert lib.tswn_battle_session_stop_reason(handle, C.byref(reason)) == 0
            assert ['running', 'finished', 'truncated'][status.value] == result['status']
            assert [None, 'winner', 'max_rounds', 'no_progress'][reason.value] == result['stop_reason']
            return dict(initial=initial, frames=frames, result=result)
        finally:
            lib.tswn_battle_session_free(handle)

    fixtures = ['1v1-0f92cb76cc37fdc5', '2v2-554f4128af707167',
                'ffa_8-16d11de1ebe1df41', '3v3v3-0ace5df17b84e26a']
    jobs = [dict(raw=(ROOT / 'crates/tswn_test/cases/runtime_stress' / (fixture + '.txt')).read_text(encoding='utf8'),
                 max_rounds=limit) for fixture in fixtures for limit in [1, 20_000]]
    wasm = json.loads(subprocess.run(['node', 'scripts/dump_battle_wasm.mjs'], cwd=ROOT,
                      input=json.dumps(jobs), capture_output=True, encoding='utf8', check=True).stdout)
    executable = ROOT / 'target/debug' / ('tswn-cli.exe' if os.name == 'nt' else 'tswn-cli')
    for i, job in enumerate(jobs):
        process = subprocess.run([str(executable), 'fight', '--jsonl', '--max-rounds', str(job['max_rounds'])],
                                 input=job['raw'], encoding='utf8', capture_output=True, check=True)
        events = [json.loads(line) for line in process.stdout.splitlines()]
        rust = dict(initial=events[0]['data'], frames=[e['data'] for e in events if e['type'] == 'frame'], result=events[-1]['data'])
        py = tswn_py.BattleSession(job['raw'], max_rounds=job['max_rounds'])
        python = dict(initial=py.initial_states(), frames=list(py), result=py.result())
        c = c_battle(**job)
        for name, actual in [('Python', python), ('C', c), ('WASM', wasm[i])]:
            assert actual == rust, f'{name} payload mismatch: {fixtures[i // 2]}, {job["max_rounds"]}'
        print(f'PASS: Rust/CLI == Python == C == WASM: {fixtures[i // 2]}, max_rounds={job["max_rounds"]}')


if __name__ == '__main__':
    main()
