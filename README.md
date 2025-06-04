# WHISPER API

A frontend service for audio transcription using WhisperX.

## Architecture

The Whisper API follows a queue-based architecture with modular components:

1. **HTTP Handler**:
   - Receives transcription requests containing audio files
   - Stores the audio file in a temporary directory
   - Adds the job to the queue manager's queue
   - Responds immediately with a URL for checking transcription status
   - Serves completed transcriptions when requested
   - Requests cleanup of files after delivering results

2. **Queue Manager**:
   - Manages a FIFO queue of transcription jobs
   - Processes one job at a time (sequential processing)
   - Invokes the WhisperX command to transcribe audio files
   - Stores results in a temporary directory
   - Tracks job status (queued, processing, completed, failed)
   - Handles cleanup of temporary files
   - Automatically removes old jobs after the retention period (configurable, default: 48 hours)

3. **Client Workflow**:
   - Client submits audio for transcription and receives a job ID
   - Client periodically checks status URL until transcription is ready
   - When ready, client downloads the transcription result
   - After successful download, temporary files are cleaned up
   - Alternatively, client can cancel a pending job if transcription is no longer needed

## Environment Variables

The application can be configured using the following environment variables:

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `WHISPER_CMD` | Path to the WhisperX command | `whisperx` |
| `WHISPER_MODELS_DIR` | Path to the models directory | `/models` |
| `WHISPER_OUTPUT_DIR` | Directory for WhisperX to store output files | `/home/llm/whisper_api/output` |
| `WHISPER_OUTPUT_FORMAT` | Default output format for transcriptions | `txt` |
| `WHISPER_TMP_FILES` | Directory for storing temporary files | `/home/llm/whisper_api/tmp` |
| `WHISPER_API_HOST` | Host address to bind the server | `127.0.0.1` |
| `WHISPER_API_PORT` | Port for the HTTP server | `8181` |
| `WHISPER_API_TIMEOUT` | Client disconnect timeout in seconds | `480` |
| `WHISPER_API_KEEPALIVE` | Keep-alive timeout in seconds | `480` |
| `WHISPER_JOB_RETENTION_HOURS` | Number of hours to keep job files before automatic cleanup | `48` |
| `WHISPER_CLEANUP_INTERVAL_HOURS` | Interval in hours between cleanup runs | `1` |
| `RUST_LOG` | Logging level (error, warn, info, debug, trace) | `info` |
| `HF_TOKEN` | Hugging Face API token for diarization models access (can alternatively be passed per-request or loaded from file) | None |
| `WHISPER_HF_TOKEN_FILE` | Path to file containing Hugging Face API token | `/home/llm/whisper_api/hf_token.txt` |

## API Endpoints

### Submit a Transcription Job

```
POST /transcribe
```

**Form Parameters**:
- `file`: Audio file to transcribe (required)
- `language`: Language code (optional, default: "fr")
- `model`: Model name (optional, default: "large-v3")
- `diarize`: Whether to apply speaker diarization (optional, default: true)
- `prompt`: Initial text prompt for transcription (optional, default: "")
- `hf_token`: Hugging Face API token for accessing diarization models (optional, if not provided will try to load from `hf_token.txt` file). Required for speaker diarization to work properly.
- `output_format`: Format of transcription output (optional, values: "srt", "vtt", "txt", "tsv", "json", "aud", default: "txt")

**Response**:
```json
{
  "job_id": "uuid-string",
  "status_url": "/transcription/uuid-string"
}
```

### Check Transcription Status

```
GET /transcription/{job_id}
```

**Response**:
```json
{
  "status": "Queued|Processing|Completed|Failed",
  "data": "Additional information (result or error)"
}
```

### Get Transcription Result

```
GET /transcription/{job_id}/result
```

**Response**:
```json
{
  "text": "Transcription text content",
  "language": "Detected language",
  "segments": [
    {
      "start": 0.0,
      "end": 2.5,
      "text": "Segment text"
    }
  ]
}
```

### Cancel a Transcription Job

```
DELETE /transcription/{job_id}
```

**Description**: Cancels a pending job and removes associated audio files. Jobs that are already processing cannot be canceled.

**Response (Success)**:
```json
{
  "success": true,
  "message": "Job canceled successfully"
}
```

**Response (Error)**:
```json
{
  "error": "Error message (job not found, already processing, etc.)"
}
```

## Test Command

```bash
curl -X POST "http://localhost:8181/transcribe" \
  -F "language=fr" \
  -F "diarize=true" \
  -F "prompt=Meeting transcript:" \
  -F "hf_token=YOUR_HUGGINGFACE_TOKEN" \
  -F "output_format=txt" \
  -F "file=@/path/to/audio.wav"
```

Note: The maximum file size accepted is 512 MB.

### Examples

#### Disable Speaker Diarization
```bash
curl -X POST "http://localhost:8181/transcribe" \
  -F "diarize=false" \
  -F "file=@/path/to/audio.wav"
```

#### Add Initial Prompt
```bash
curl -X POST "http://localhost:8181/transcribe" \
  -F "prompt=This is an interview between John and Sarah:" \
  -F "file=@/path/to/audio.wav"
```

#### Use Diarization with Hugging Face Token
```bash
curl -X POST "http://localhost:8181/transcribe" \
  -F "diarize=true" \
  -F "hf_token=YOUR_HUGGINGFACE_TOKEN" \
  -F "file=@/path/to/audio.wav"
```

Note: If you don't provide the `hf_token` parameter, the system will attempt to read the token from the file specified by `WHISPER_HF_TOKEN_FILE` (default: `/home/llm/whisper_api/hf_token.txt`). If no token is available (file missing, empty, or unreadable) and diarization is requested, diarization will be automatically disabled with a warning in the logs. This ensures transcription jobs can proceed even without a token, but without speaker identification.

#### Specify Output Format
```bash
curl -X POST "http://localhost:8181/transcribe" \
  -F "output_format=srt" \
  -F "file=@/path/to/audio.wav"
```

## File Structure

The API is organized into modular components:

- **handlers/**: HTTP request handlers and form processing
- **models.rs**: Data structures for requests and responses
- **file_utils.rs**: File operations and resource management
- **queue_manager.rs**: Job queue and transcription processing
- **config.rs**: Application configuration 
- **error.rs**: Error handling and HTTP responses

## Resources

- WhisperX: https://github.com/m-bain/whisperX
- [WhisperX Documentation](whisperx.md)
