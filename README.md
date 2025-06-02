# WHISPER API

A frontend service for audio transcription using WhisperX.

## Architecture

The Whisper API follows a queue-based architecture:

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
| `WHISPER_TMP_FILES` | Directory for storing temporary files | `/tmp/whisper_api` |
| `WHISPER_API_HOST` | Host address to bind the server | `127.0.0.1` |
| `WHISPER_API_PORT` | Port for the HTTP server | `8181` |
| `WHISPER_API_TIMEOUT` | Client disconnect timeout in seconds | `480` |
| `WHISPER_API_KEEPALIVE` | Keep-alive timeout in seconds | `480` |
| `WHISPER_JOB_RETENTION_HOURS` | Number of hours to keep job files before automatic cleanup | `48` |
| `WHISPER_CLEANUP_INTERVAL_HOURS` | Interval in hours between cleanup runs | `1` |
| `RUST_LOG` | Logging level (error, warn, info, debug, trace) | `info` |

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
  -F "file=@/path/to/audio.wav"
```

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

## Resources

- WhisperX: https://github.com/m-bain/whisperX
- [WhisperX Documentation](whisperx.md)
