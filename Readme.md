This is 2d minecraft written in Rust and Python.

## Members
- 35 Thad Choyrum ([@itscrystalline](https://github.com/itscrystalline)): Game Logic, Server Networking 
- 22 Ashira Saelim ([@FujiForm2023](https://github.com/FujiForm2023)): Client Networking, Client Logic
- 28 Korawit Kumpakarn ([@napookooo](https://github.com/napookooo)): Game Logic
- 25 Nuntanut Poonpayap ([@NessShadow](https://github.com/NessShadow)): Art, Client
- 41 Pannathorn Hanjirasawat ([@koolmakmak](https://github.com/koolmakmak)): Art, Client

## Running
> [!NOTE]
> All paths are relative to the project root.
### Rust Server
Install rustup from https://rustup.rs/, then run
`cargo run` to start the server immediately, or `cargo build`/`cargo build --release` to compile with/without dev symbols.

The compiled binary will be in `target/debug/yourcraft-server(.exe)` or `target/release/yourcraft-server(.exe)`, depending on whether you compile with the `--release` flag or not.

Simply run `yourcraft-server(.exe) [options..]` or double-click in Explorer on Windows.

The server will start on port `8475` by default.

**Options**

```shell
> ./yourcraft-server --help
KOSEN-KMITL Programming 4 Final Project - A 2d sandbox game server.

Usage: yourcraft-server [OPTIONS] <COMMAND>

Commands:
  empty    An empty world with nothing in it
  flat     A flat grass world
  terrain  Perlin noise based terrain
  help     Print this message or the help of the given subcommand(s)

Options:
  -p, --port <PORT>                  The port to use for clients to connect to [default: 8475]
      --world-width <WORLD_WIDTH>    The world's horizontal size [default: 1024]
      --world-height <WORLD_HEIGHT>  The world's vertical size [default: 256]
  -c, --chunk-size <CHUNK_SIZE>      The size of each chunk the world subdivides into [default: 16]
      --spawn-point <SPAWN_POINT>    The x coordinate of the center of spawn point. Defaults to the center of the world. (e.g. world_width / 2)
      --spawn-range <SPAWN_RANGE>    The spawn range that players will spawn around spawn_point [default: 16]
  -n, --no-console                   Disables the command console
      --debug                        Enables Debug Logging
      --no-heartbeat                 Disables sending heartbeat packets to connected clients
  -h, --help                         Print help
  -V, --version                      Print version
```

### Python Client
Using Python 3.12.7, activate a virtualenv by
```shell
python -m venv .venv
source .venv/bin/activate
```
or on Windows:
```shell
python -m venv .venv
.venv\Scripts\activate
```
If you are running on Powershell and get an error message about execution policies, run
`Set-ExecutionPolicy Unrestricted` before running `.venv\Scripts\activate`.

If the shell complains that `python` is not found, try a different alias. examlpes include
`python3`, `python3.12`, `py` and `py3`.

Then, run `pip install -r requirements.txt` to install the dependencies.

Finally, simply run `python py/main.py` to start the client.

## Network Spec
This project uses Python's `pickle` format for communication between the server and the client.

The client side will use the `pickle` Python module, while the server side will use the `serde_pickle` Rust crate.

Each packet is defined as follows:

```
// In Rust (rs/network.rs)
pub enum PacketTypes {
    // -- snip --
    [Packet Name] {
        [Packet Data] // if packet has no data just put nothing in the block
    }
    // -- snip --
}

# In Python (py/network.py)
class [Packet Name](packet):
    def __init__([properties..]):
        self.[property] = [property]
                      ..

    # or if the packet has no data
    pass
```

In Rust, the data recieved from the Network Thread will already be a variant in `PacketTypes`, simply destructure and use it's values. To send, obtain a pipe to the Network Thread (of type `network::ToNetwork`) and call the `network::encode_and_send!(ToNetwork, PacketTypes, SocketAddr)` macro, with the pipe, the packet (variant of `PacketTypes`), and the `SocketAddr` of the target client. 

In Python, the data recieved from `ServerConnection#recv()` will be a dict with `t` being the packet name, and `data` being the packet's contents. To send, simply call `ServerConnection#send(Packet)` with the packet class as the argument.   

### Notice On Updates
The client **must** request the server to load/unload chunks. the server **will only** broadcast block/player updates that are in the client's loaded area.

The client must also respond to the server's heartbeat packets, else the connection is deemed "lost" and the client is
kicked from the server.

### Packet Name & Content List 
✅: fully implemented & tested, 🟨: implemented but not tested, ⬛: not implemented

(Server, Client)
```
✅✅ [C2S] ClientHello (name: str)                                                                                           
✅✅ [S2C] ServerSync (player_id: int, world_width: int, world_height: int, chunk_size: int, spawn_x: float, spawn_y: float) 
✅✅ [C2S] ClientRequestChunk (chunk_coords_x: int, chunk_coords_y: int)                                                     
✅✅ [S2C] ServerChunkResponse (chunk: Chunk, see world.rs for impl)                                                         
✅✅ [C2S] ClientUnloadChunk (chunk_coords_x: int, chunk_coords_y: int)                                                      
🟨🟨 [S2C] ServerPlayerJoin (player_name: str, player_id: int)                                                               
✅✅ [S2C] ServerPlayerEnterLoaded (player_name: str, player_id: int, pos_x: float, pos_y: float)                            
✅✅ [S2C] ServerPlayerLeaveLoaded (player_name: str, player_id: int)                                                        
🟨🟨 [S2C] ServerPlayerLeave (player_name: str, player_id: int)                                                              
✅✅ [C2S] ClientGoodbye ()                                                                                                 
✅✅ [C2S] ClientPlaceBlock (x: int, y: int)
✅✅ [S2C] ServerUpdateBlock (block: Block Enum as int, x: int, y: int)                                                     
✅✅ [C2S] ClientPlayerXVelocity (vel_x: float)                                                                                 
✅✅ [C2S] ClientPlayerJump ()
✅✅ [C2S] ClientPlayerRespawn ()
✅✅ [S2C] ServerPlayerUpdatePos (player_id: int, pos_x: float, pos_y: float)                                               
✅✅ [S2C] ServerKick (msg: str)                                                                                            
✅✅ [S2C] ServerHeartbeat                                                                                                  
✅✅ [C2S] ClientHeartbeat
🟨⬛️ [S2C] ServerSendMessage (player_id: int, player_name: str, msg: str)
✅✅ [C2S] ClientSendMessage (msg: str)
✅✅ [C2S] ClientBreakBlock (x: int, y: int)
⬛️⬛[C2S] ClientTryAttack (player_id: int)
✅✅️ [C2S] ClientChangeSlot (slot: int)
⬛️⬛️ [S2C] ServerUpdateHealth (health: int)
🟨⬛️ [S2C] ServerUpdateInventory (inv: list of ItemStack)
```

### Lifecycle Overview
```
Client               <--->               Server

                     Start
ClientHello ---------------------------------->
<----------------------------------- ServerSync
                      ...
<----------------------------- ServerPlayerJoin
<---------------------------- ServerPlayerLeave
<----------------------------------- ServerKick
                      ...
ClientRequestChunk --------------------------->
<-------------------------- ServerChunkResponse
                      ...
<---------------------- ServerPlayerEnterLoaded
<---------------------- ServerPlayerLeaveLoaded
                      ...
ClientPlaceBlock ----------------------------->
<---------------------------- ServerUpdateBlock
                      ...
ClientPlayerXVelocity ------------------------>
ClientPlayerJump ----------------------------->
<------------------------ ServerPlayerUpdatePos
                      ...
ClientUnloadChunk ---------------------------->
                      ...
ClientGoodbye -------------------------------->
                      End
```
