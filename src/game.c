#include <stdio.h>
#include "SDL2/SDL.h"
#include <time.h>
#include <string.h>
// our function 
#include "game.h"
#include "collision.h"
#include "struct.h"

void loadGame(GameState *mState){

  // Initilize player
  mState->man.x = 960;
  mState->man.y = 540;
  mState->man.size_x = 100;
  mState->man.size_y = 200;

  mState->keyState.space = 0;
  mState->keyState.o = 0;
  mState->keyState.p = 0;

  // Initilize types, size and cord of platforms
  mState->platform_used = 0;
  mState->platform_used += 40;
  for (int i=0; i<mState->platform_used; i++){
    mState->plat[i].size_x = 100;
    mState->plat[i].size_y = 100;
    mState->plat[i].x = -200 + 100*i;
    mState->plat[i].y = 800;
  }
  // Initilize platform collision
  for (int i=0; i<mState->platform_used; i++){
    mState->plat[i].cx[0] = mState->plat[i].x;
    mState->plat[i].cy[0] = mState->plat[i].y;
    mState->plat[i].cx[1] = mState->plat[i].x+mState->plat[i].size_x;
    mState->plat[i].cy[1] = mState->plat[i].y+mState->plat[i].size_y;
  }
}


void events(GameState *mState){

  for (int i=0; i<mState->platform_used; i++){
    int collision = playerCollision(&mState->man, mState->plat[i].cx, mState->plat[i].cy);
    printf("%d\n", collision);
  }

  mState->man.cx[0] = mState->man.x;
  mState->man.cy[0] = mState->man.y;
  mState->man.cx[1] = mState->man.x+mState->man.size_x;
  mState->man.cy[1] = mState->man.y+mState->man.size_y;



}

int processInputs(SDL_Window *window, GameState *mState){
  SDL_Event event;
  int done = 0;

  while(SDL_PollEvent(&event)){
    switch(event.type){
      case SDL_WINDOWEVENT_CLOSE:{
        if(window){
          SDL_DestroyWindow(window);
          window = NULL;
          done = 1;
        }
      }
      break;
      case SDL_KEYDOWN:{
        switch (event.key.keysym.sym) {
          case SDLK_ESCAPE:
            done = 1;
          case SDLK_p:
            if (!mState->keyState.p){
              mState->keyState.p = 1;
            }
            else {
              mState->keyState.p = 0;
            }
          case SDLK_o:
            if (!mState->keyState.o){
              mState->keyState.o = 1;
            }
            else {
              mState->keyState.o = 0;
            }
          case SDLK_t:
            printf("vx: %.2f\n", mState->man.vx);
            printf("vy: %.2f\n", mState->man.vy);
          break;
        }
      }
    break;
    case SDL_QUIT:
      done = 1;
      break;
    }
  }

  const Uint8 *state = SDL_GetKeyboardState(NULL);
  if(state[SDL_SCANCODE_A]){
    // playerMoveLeft(&mState->man);
    mState->man.x -= 10;
  }
  if(state[SDL_SCANCODE_D]){
    // playerMoveRight(&mState->man);
    mState->man.x += 10;
  }
  if(state[SDL_SCANCODE_W]){
    // playerMoveLeft(&mState->man);
    mState->man.y -= 10;
  }
  if(state[SDL_SCANCODE_S]){
    // playerMoveRight(&mState->man);
    mState->man.y += 10;
  }
  if(state[SDL_SCANCODE_R]){
    mState->man.x = 500;   mState->man.y = 500;
  }

  return done;
}
