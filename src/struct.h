#ifndef STRUCT
#define STRUCT

typedef struct{
  float x, y;
  short lookDirection;
  float cx[2], cy[2];
  float size_x, size_y;

  // movement
  float vx, vy;

  // action
  int attack;
  float att_sx, att_sy;
  float att_x, att_y;

} Man;

typedef struct{
  float x, y;
  float cx[2], cy[2];
  float size_x, size_y;
} Platform_x;

typedef struct {
  int space;
  int o;
  int p;
} Keyboard;

typedef struct {
  Man man;

  // Grounds
  Platform_x plat[100];
  int platform_used;

  SDL_Renderer *renderer;

  // keyboard state
  Keyboard keyState;

} GameState;

#define GRAVITY 0.48f


#endif // !
