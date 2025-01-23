# Component Class
# A component is contained in Entity, defined data in entity itself

import dataclasses
from typing import Any, TypeVar
import abc
from immutabledict import ImmutableDict

T = TypeVar('T', bound=dict)

@dataclasses.dataclass
class Component(abc.ABC):
    __data: ImmutableDict[str, Any] = dataclasses.field(init=False)

    def __post_init__(self):
        self.__data = ImmutableDict({k: v for k, v in self.__dict__.items() if k != '__data'})

    def setData(self, new_data: T) -> None:
        for key, value in new_data.items():
            if key in self.__data:
                self.__data[key] = value
            else:
                raise KeyError(f"Key '{key}' does not exist and cannot be added.")

    def getData(self) -> ImmutableDict[Any, Any]:
        return self.__data

# How to use

# @dataclasses.dataclass
# class PositionComponent(Component):
#     __x : float
#     __y : float
# That's it
