import pickle
import socket

from time import sleep, time

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
class HelloPacket(Packet):
    def __init__(self):
        super().__init__(0)
        self.timestamp = int(time() * 1000)

ip_port = ("127.0.0.1", 8475)
buf_size = 1024

# sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

# while True:
#     pkt = HelloPacket()
#
#     sock.sendto(pkt.serialize(), ip_port)
#
#     sleep(1)