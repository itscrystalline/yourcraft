import pickle
import random
import socket

from time import sleep, time

HELLO = 1
PLAYER_COORDINATES = 2

class Packet:
    def __init__(self, packet_type):
        self.t = packet_type
    def serialize(self):
        serialized_inner = pickle.dumps(self.to_dict())
        return pickle.dumps({"t": self.t, "data": serialized_inner})
    def to_dict(self):
        d = dict(self.__dict__)
        d.pop("t")
        return d
class Connection:
    def __init__(self, ip: str, port: int):
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.ip_port = (ip, port)
    def send(self, packet: Packet):
        self.socket.sendto(packet.serialize(), self.ip_port)


class HelloPacket(Packet):
    def __init__(self, timestamp):
        super().__init__(HELLO)
        self.timestamp = timestamp

class PlayerCoordinates(Packet):
    def __init__(self, x, y):
        super().__init__(PLAYER_COORDINATES)
        self.x = x
        self.y = y

if __name__ == "__main__":
    type = int(input("test type:"))
    conn = Connection("127.0.0.1", 8475)

    while True:
        if type == 0:
            conn.send(HelloPacket(int(time() * 1000)))
        elif type == 1:
            conn.send(PlayerCoordinates(random.randint(-100, 100), random.randint(-100, 100)))

        sleep(1)