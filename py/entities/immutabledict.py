# For component

from collections.abc import Mapping


class ImmutableDict(Mapping):
    def __init__(self, data: dict) -> None:
        self._data = data

    def __getitem__(self, key):
        return self._data[key]

    def __iter__(self):
        return iter(self._data)

    def __len__(self):
        return len(self._data)

    def __setitem__(self, key, value):
        if key not in self._data:
            raise KeyError(f"Key '{key}' does not exist and cannot be added.")
        self._data[key] = value

    def __delitem__(self, key):
        raise KeyError(f"Key '{key}' cannot be deleted.")

    def __repr__(self):
        return f"ImmutableDict({self._data!r})"
