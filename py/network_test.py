import pickle
import socket

HELLO = 1
PLAYER_COORDINATES = 2
GOODBYE = 10
KICK = 16
HEARTBEAT_SERVER = 17
HEARTBEAT_CLIENT = 18

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


class ServerConnection:
    def __init__(self, ip: str, port: int = 8475):
        self.socket = socket.socket(family=socket.AF_INET, type=socket.SOCK_DGRAM)
        self.ip_port = (ip, port)
    def send(self, packet: Packet):
        self.socket.sendto(packet.serialize(), self.ip_port)

    def recv(self):
        packet = pickle.loads(self.socket.recv(1024*16))
        packet['data'] = pickle.loads(packet['data'])
        return packet


class Hello(Packet):
    def __init__(self, username):
        super().__init__(HELLO)
        self.name = username


class Goodbye(Packet):
    def __init__(self):
        super().__init__(GOODBYE)


class Heartbeat(Packet):
    def __init__(self):
        super().__init__(HEARTBEAT_CLIENT)

class ClientRequestChunk (Packet):
    def __init__(self, x, y):
        super().__init__(3)
        self.chunk_coords_x = x
        self.chunk_coords_y = y

if __name__ == "__main__":
    respond_to_heartbeat = True if input("respond to heartbeat?") == "y" else False
    conn = ServerConnection("127.0.0.1")

    conn.send(Hello("test"))
    INIT_DATA = conn.recv()

    while True:
        receiving = conn.recv()
        print(receiving)
        if receiving['t'] == KICK:
            print("kicked because", receiving['data']['msg'])
            exit(0)
        elif receiving['t'] == HEARTBEAT_SERVER:
            print("heartbeat received")
            conn.send(Heartbeat()) if respond_to_heartbeat else None
        conn.send(ClientRequestChunk (0, 0))
