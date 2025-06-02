# WhisperX - Command Line Reference

WhisperX is a fast automatic speech recognition (ASR) with word-level timestamps and speaker diarization.

## Basic Usage

```bash
whisperx audio.wav --model medium
```

## Positional Arguments

| Argument | Description |
|----------|-------------|
| `audio` | Audio file(s) to transcribe |

## Optional Arguments

### General Options

| Argument | Default | Description |
|----------|---------|-------------|
| `-h, --help` | | Show help message and exit |
| `--model MODEL` | `small` | Name of the Whisper model to use |
| `--model_cache_only` | `False` | If True, will not attempt to download models, instead using cached models from `--model_dir` |
| `--model_dir` | `~/.cache/whisper` | The path to save model files |
| `--device` | `cuda` | Device to use for PyTorch inference |
| `--device_index` | `0` | Device index to use for FasterWhisper inference |
| `--batch_size` | `8` | The preferred batch size for inference |
| `--compute_type` | `float16` | Compute type for computation (options: `float16`, `float32`, `int8`) |

### Output Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--output_dir, -o` | `.` | Directory to save the outputs |
| `--output_format, -f` | `all` | Format of the output file (options: `all`, `srt`, `vtt`, `txt`, `tsv`, `json`, `aud`) |
| `--verbose` | `True` | Whether to print out progress and debug messages |

### Transcription Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--task` | `transcribe` | Whether to perform speech recognition (`transcribe`) or translation to English (`translate`) |
| `--language` | `None` | Language spoken in the audio. If not specified, performs language detection |
| `--temperature` | `0` | Temperature to use for sampling |
| `--best_of` | `5` | Number of candidates when sampling with non-zero temperature |
| `--beam_size` | `5` | Number of beams in beam search (only when temperature is zero) |

### Alignment Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--align_model` | `None` | Name of phoneme-level ASR model to do alignment |
| `--interpolate_method` | `nearest` | Method to assign timestamps to non-aligned words (`nearest`, `linear`, `ignore`) |
| `--no_align` | `False` | Do not perform phoneme alignment |
| `--return_char_alignments` | `False` | Return character-level alignments in the output JSON file |

### Voice Activity Detection (VAD) Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--vad_method` | `pyannote` | VAD method to be used (`pyannote` or `silero`) |
| `--vad_onset` | `0.5` | Onset threshold for VAD; reduce if speech is not being detected |
| `--vad_offset` | `0.363` | Offset threshold for VAD; reduce if speech is not being detected |
| `--chunk_size` | `30` | Chunk size for merging VAD segments |

### Diarization Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--diarize` | `False` | Apply diarization to assign speaker labels to each segment/word |
| `--min_speakers` | `None` | Minimum number of speakers in audio file |
| `--max_speakers` | `None` | Maximum number of speakers in audio file |

### Advanced Decoding Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--patience` | `1.0` | Optional patience value for beam decoding |
| `--length_penalty` | `1.0` | Optional token length penalty coefficient (alpha) |
| `--suppress_tokens` | `-1` | Comma-separated list of token IDs to suppress during sampling |
| `--suppress_numerals` | `False` | Whether to suppress numeric and currency symbols |
| `--initial_prompt` | `None` | Optional text to provide as a prompt for the first window |
| `--condition_on_previous_text` | `False` | Whether to provide previous output as prompt for next window |

### Performance and Format Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--fp16` | `True` | Whether to perform inference in fp16 |
| `--max_line_width` | `None` | Maximum characters in a line before breaking |
| `--max_line_count` | `None` | Maximum number of lines in a segment |
| `--highlight_words` | `False` | Underline each word as it is spoken in SRT and VTT |
| `--segment_resolution` | `sentence` | Resolution for segmentation (`sentence` or `chunk`) |
| `--threads` | `0` | Number of threads used by torch for CPU inference |

### Fallback Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--temperature_increment_on_fallback` | `0.2` | Temperature increase when decoding fails |
| `--compression_ratio_threshold` | `2.4` | Treat decoding as failed if gzip compression ratio exceeds this |
| `--logprob_threshold` | `-1.0` | Treat decoding as failed if average log probability is below this |
| `--no_speech_threshold` | `0.6` | Consider segment as silence if probability of `<|nospeech|>` exceeds this |

### Other Options

| Argument | Default | Description |
|----------|---------|-------------|
| `--hf_token` | `None` | Hugging Face Access Token to access PyAnnote gated models |
| `--print_progress` | `False` | Whether to print progress in transcribe() and align() methods |
| `--version, -V` | | Show WhisperX version information and exit |
| `--python-version, -P` | | Show Python version information and exit |

## Supported Languages

WhisperX supports numerous languages including:
- Afrikaans (af)
- Arabic (ar)
- Chinese (zh)
- Czech (cs)
- Dutch (nl)
- English (en)
- French (fr)
- German (de)
- Italian (it)
- Japanese (ja)
- Korean (ko)
- Polish (pl)
- Portuguese (pt)
- Russian (ru)
- Spanish (es)
- Swedish (sv)
- Turkish (tr)
- Ukrainian (uk)
- Vietnamese (vi)

*And many more...*