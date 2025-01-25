This is 2d minecraft written in rust and python.

## Running
> [!NOTE]
> All paths are relative to the project root.
### Rust Server
Install rustup from https://rustup.rs/, then run
`cargo run` to start the server immediately, or `cargo build`/`cargo build --release` to compile with/without dev symbols.

The compiled binary will be in `target/debug/yourcraft-server(.exe)` or `target/release/yourcraft-server(.exe)`, depending on whether you compile with the `--release` flag or not.

Simply run `yourcraft-server(.exe)` or double-click in Explorer on Windows.

The server will start on port `8475` by default.

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
This project uses python's `pickle` format for communication between the server and the client.

The client side will use the `pickle` python module, while the server side will use the `serde_pickle` Rust crate.

The generic packet will be a serialized python dict / rust struct defined as follows:
```
{
  t: int/u8 ( u8 <-> PacketTypes Enum via the Into trait)
  data: list/Vec<u8> (Serialized packet data as a byte array)
}
```

each packet will have its own ID used to identify what the packet is. this is signified as `t`.

the data that each packet carries is serialized and stored as `data`, which will also have to be unpickled into a python `dict`.

the reason this is done is to make sure that the server/client can decode all incoming packets properly, and can discard any invalid ones.

### Notice On Updates
The client **must** request the server to load/unload chunks. the server **will only** broadcast block/player updates that are in the client's loaded area.

### Packet ID List & content 
✅: fully implmented & tested, 🟨: implemented but not tested, ⬛: not implemented

(Server, Client)
```
0: Invalid Packet.
1: [C2S] ClientHello (name: str)                                                           ✅✅
2: [S2C] ServerSync (player_id: int, world_width: int, world_height: int, chunk_size: int) 🟨⬛
3: [C2S] ClientRequestChunk (chunk_coords_x: int, chunk_coords_y: int)                     🟨⬛
4: [S2C] ServerChunkResponse (chunk: Chunk, see world.rs for impl)                         🟨⬛
5: [C2S] ClientUnloadChunk (chunk_coords_x: int, chunk_coords_y: int)                      🟨⬛
6: [S2C] ServerPlayerJoin (player_name: str, player_id: int)                               🟨⬛
7: [S2C] ServerPlayerEnterLoaded (player_name: str, player_id: int)                        🟨⬛
8: [S2C] ServerPlayerLeaveLoaded (player_name: str, player_id: int)                        🟨⬛
9: [S2C] ServerPlayerLeave (player_name: str, player_id: int)                              🟨⬛
10: [C2S] ClientGoodbye ()                                                                 🟨⬛
11: [C2S] ClientPlaceBlock (block: Block Enum as int, x: int, y: int)                      🟨⬛
12: [S2C] ServerUpdateBlock (block: Block Enum as int, x: int, y: int)                     🟨⬛
13: [C2S] ClientPlayerMoveX (pos_x: float)                                                 ⬛⬛
13: [C2S] ClientPlayerJump ()                                                              ⬛⬛
14: [S2C] ServerPlayerUpdatePos (player_id: int, pos_x: float, pos_y: float)               ⬛⬛
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
ClientPlayerMoveX ---------------------------->
ClientPlayerJump ----------------------------->
<------------------------ ServerPlayerUpdatePos
                      ...
ClientUnloadChunk ---------------------------->
                      ...
ClientGoodbye -------------------------------->
                      End
```