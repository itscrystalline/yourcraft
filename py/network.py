import pickle
import socket

HELLO = "ClientHello"
# PLAYER_COORDINATES
# CHUNK_REQUEST
CHUNK_UPDATE = "ServerChunkResponse"
# CHUNK_UNLOAD
PLAYER_JOIN = "ServerPlayerJoin"
PLAYER_ENTER_LOAD = "ServerPlayerEnterLoaded"
PLAYER_LEAVE_LOAD = "ServerPlayerLeaveLoaded"
# PLAYER_LEAVE
# GOODBYE
# PLACE_BLOCK
UPDATE_BLOCK = "ServerUpdateBlock"
BATCH_UPDATE_BLOCK = "ServerBatchUpdateBlock"
# PLAYER_JUMP
PLAYER_UPDATE_POS = "ServerPlayerUpdatePos"
KICK = "ServerKick"
HEARTBEAT_SERVER = "ServerHeartbeat"
# HEARTBEAT_CLIENT
SERVER_MESSAGE = "ServerSendMessage"
# CLIENT_MESSAGE
# BREAK_BLOCK
# ATTACK_PLAYER
# CHANGE_SLOT
UPDATE_HEALTH = "ServerUpdateHealth"
UPDATE_INVENTORY = "ServerUpdateInventory"



class Packet:
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
        packet = pickle.loads(self.socket.recv(1024 * 16))
        return {
            "t": next(iter(packet.keys())),
            "data": next(iter(packet.values()))
        }


class ClientHello(Packet):
    def __init__(self, username):
        self.name = username


class ClientGoodbye(Packet):
    pass


class ClientHeartbeat(Packet):
    pass


class ClientRequestChunk(Packet):
    def __init__(self, x, y):
        self.chunk_coords_x = x
        self.chunk_coords_y = y


class ClientPlaceBlock(Packet):
    def __init__(self, x, y):
        self.x: int = x
        self.y: int = y


class ClientBreakBlock(Packet):
    def __init__(self, x, y):
        self.x: int = x
        self.y: int = y


class ClientPlayerXVelocity(Packet):
    def __init__(self, x):
        self.vel_x = x


class ClientPlayerJump(Packet):
    pass


class ClientUnloadChunk(Packet):
    def __init__(self, x, y):
        self.chunk_coords_x = x
        self.chunk_coords_y = y


class ClientSendMessage(Packet):
    def __init__(self, msg):
        self.msg = msg


class ClientTryAttack(Packet):
    def __init__(self, player_id):
        self.player_id = player_id


class ClientChangeSlot(Packet):
    def __init__(self, slot):
        self.slot = slot




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
