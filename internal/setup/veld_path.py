import os as _os, sys as _sys
_veld_root = _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), ".")
if _veld_root not in _sys.path:  # veld:generated-path
    _sys.path.insert(0, _veld_root)
