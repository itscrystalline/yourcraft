#ifndef LOADGAME_H
#define LOADGAME_H

#include "struct.h"

void loadGame(GameState *mState);
void events(GameState *mState);
int processInputs(SDL_Window *window, GameState *mState);
int playerLimits(Man *man);

#endif
