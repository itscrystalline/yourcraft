#include <stdio.h>
#include "SDL2/SDL.h"
#include <time.h>
#include <string.h>
// our function 
#include "render.h"
#include "struct.h"
#include "collision.h"

void doRender(SDL_Renderer *renderer, GameState *mState){
  
  // clean screen
  SDL_SetRenderDrawColor(renderer, 50, 50, 255, 255);
  SDL_RenderClear(renderer);
  SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);

  SDL_SetRenderDrawColor(renderer, 200, 12, 255, 255);
  SDL_Rect player_rect = { mState->man.x, mState->man.y, mState->man.size_x, mState->man.size_y };
  SDL_RenderFillRect(renderer, &player_rect);

  // render platform
  SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);
  for (int i=0; i<mState->platform_used; i++){
    SDL_Rect platform_x_rect = { mState->plat[i].x, mState->plat[i].y, mState->plat[i].size_x, mState->plat[i].size_y };
    SDL_RenderFillRect(renderer, &platform_x_rect);
  }

  SDL_RenderPresent(renderer);
}


