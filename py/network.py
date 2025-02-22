import pickle
import socket

HELLO = "ClientHello"
PLAYER_COORDINATES = 2
CHUNK_REQUEST = 3
CHUNK_UPDATE = "ServerChunkResponse"
CHUNK_UNLOAD = 5
PLAYER_JOIN = 6
PLAYER_ENTER_LOAD = 7
PLAYER_LEAVE_LOAD = 8
PLAYER_LEAVE = 9
GOODBYE = 10
PLACE_BLOCK = 11
UPDATE_BLOCK = 12
PLAYER_MOVE = 13
PLAYER_JUMP = 14
PLAYER_UPDATE_POS = "ServerPlayerUpdatePos"
KICK = "ServerKick"
HEARTBEAT_SERVER = "ServerHeartbeat"
HEARTBEAT_CLIENT = 18


class Packet:
    def __init__(self, packet_type):
        self.t = packet_type

    def serialize(self):
        contents = self.__dict__
        name = type(self).__name__
        return pickle.dumps((name, contents))


class ServerConnection:
    def __init__(self, ip: str, port: int = 8475):
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.ip_port = (ip, port)

    def send(self, packet: Packet):
        self.socket.sendto(packet.serialize(), self.ip_port)

    def recv(self):
        packet = pickle.loads(self.socket.recv(1024*16))
        return {"t": next(iter(packet.keys())), "data": next(iter(packet.values()))}


class ClientHello(Packet):
    def __init__(self, username):
        super().__init__(HELLO)
        self.name = username


class ClientGoodbye(Packet):
    def __init__(self):
        super().__init__(GOODBYE)


class ClientHeartbeat(Packet):
    def __init__(self):
        super().__init__(HEARTBEAT_CLIENT)


class ClientRequestChunk(Packet):
    def __init__(self, x, y):
        super().__init__(CHUNK_REQUEST)
        self.chunk_coords_x = x
        self.chunk_coords_y = y


class ClientPlaceBlock(Packet):
    def __init__(self, block, x, y):
        super().__init__(PLACE_BLOCK)
        self.block : int = block
        self.x : int = x
        self.y : int = y


class ClientPlayerMoveX(Packet):
    def __init__(self, x):
        super().__init__(PLAYER_MOVE)
        self.pos_x = x


class ClientPlayerJump(Packet):
    def __init__(self):
        super().__init__(PLAYER_JUMP)


class ClientUnloadChunk(Packet):
    def __init__(self, x, y):
        super().__init__(CHUNK_UNLOAD)
        self.chunk_coords_x = x
        self.chunk_coords_y = y

def test():
    conn = ServerConnection("127.0.0.1")

    conn.send(ClientHello("test"))
    INIT_DATA = conn.recv()

    print(INIT_DATA)

    conn.send(ClientRequestChunk(0, 0))
    chunk = conn.recv()['data']['chunk']
    # print(chunk)
    print(chunk['blocks'].__len__())

    while True:
        receiving = conn.recv()
        print(receiving)
        if receiving['t'] == KICK:
            print("kicked because", receiving['data']['msg'])
            exit(0)
        elif receiving['t'] == HEARTBEAT_SERVER:
            print("heartbeat received")
            conn.send(ClientHeartbeat())


if __name__ == "__main__":
    test()
