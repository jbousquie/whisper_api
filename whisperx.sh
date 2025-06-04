#!/bin/bash

# Chemin absolu vers le répertoire de whisperx
PROJECT_DIR="/home/llm/whisperx"

# Chemin absolu vers l'environnement virtuel : venv
VENV_DIR="$PROJECT_DIR/venv"

# Chemin absolu vers la commande whisperx
PYTHON_SCRIPT="$VENV_DIR/bin/whisperx"

# --- Activaction de l'environnement virtuel ---
source "$VENV_DIR/bin/activate"

# --- Exécution du programme Python avec tous les arguments passés au script ---
python "$PYTHON_SCRIPT" "$@"

# --- Désactivation  l'environnement virtuel ---
deactivate
