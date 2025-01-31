# VERY BAD MAKEFILE, KIDS LOOK AWAY

CC=gcc
CFLAGS=-std=c18 -Wall -Werror -O7
SDLFLAGS = -lSDL2
SRCDIR=src
BUILDDIR=build

SRCS = $(wildcard $(SRCDIR)/*.c)
OBJS := $(subst $(SRCDIR), $(BUILDDIR), $(patsubst %.c, %.o, $(SRCS)))
BIN = main

all: $(BIN)

$(BIN): $(OBJS)
	$(CC) -o $(BIN) $(OBJS) $(CFLAGS) $(SDLFLAGS)

$(BUILDDIR)/%.o: $(SRCDIR)/%.c
	@ $(CC) -c -o $@ $< $(CFLAGS) $(SDLFLAGS)

clean:
	rm -r $(BUILDDIR)/*
	rm main
