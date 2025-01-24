# Component Class
# A component is contained in Entity, defined data in entity itself

import dataclasses
from typing import Any, final
import abc


@dataclasses.dataclass
class Component(abc.ABC):
    __initialized: bool = False

    def __post_init__(self):
        self.__initialized = True

    # Component behaviour
    @final
    def getVariable(self, *__keys: tuple[str]) -> Any:
        __dict = {}
        if __keys[0] == 1:
            return self.__dict__
        for __key in __keys:
            __dict[__key] = self.__dict__[__key]
        if __dict.__len__() == 1:
            return __dict[__keys[0]]
        return __dict

    @final
    def setVariable(self, __dict: dict | None = None, **__kwargs: Any) -> None:
        if __dict is None:
            if __kwargs.__len__() == 0:
                return
            for __key, __value in __kwargs.items():
                self.__dict__.__setitem__(k=__key, v=__value)
        elif isinstance(__dict, dict) or isinstance(__dict.__class__, type) and issubclass(__dict.__class__, dict):
            if __kwargs.__len__() != 0:
                raise ValueError("__kwargs.__len__() is not 0")
            for __key, __value in __dict:
                self.__dict__.__setitem__(k=__key, v=__value)
        else:
            raise TypeError("__dict is not dict nor subclass of dict")

    @final
    def __setattr__(self, __key: str, __value: Any) -> None:
        if self.__initialized:
            if __key in self.__dict__:
                super().__setattr__(__key, __value)
            else:
                raise AttributeError(f"Component can not add new attribute {__key}")
        else:
            super().__setattr__(__key, __value)

    @final
    def __delattr__(self, __key: str) -> None:
        raise AttributeError(f"Component can not delete attribute: '{__key}'")

# How to use

# @dataclasses.dataclass
# class PositionComponent(Component):
#     __x : float
#     __y : float
# That's it
