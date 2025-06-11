# WHISPER API

A frontend service for audio transcription using WhisperX with authentication.

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
   - Supports both sequential and concurrent processing modes (configurable via `ENABLE_CONCURRENCY`)
   - Configurable number of concurrent jobs when concurrency is enabled (`MAX_CONCURRENT_JOBS`)
   - Invokes the WhisperX command to transcribe audio files
   - Stores results in a temporary directory
   - Tracks job status (queued, processing, completed, failed)
   - Handles cleanup of all job files (both temporary and output files)
   - Automatically removes old jobs after the retention period (configurable, default: 48 hours)

3. **Client Workflow**:
   - Client authenticates by including an Authorization header with a Bearer token
   - Client submits audio for transcription and receives a job ID
   - Client periodically checks status URL until transcription is ready
   - When ready, client downloads the transcription result
   - After successful download, all files (both temporary and output files) are cleaned up
   - Alternatively, client can cancel a pending job if transcription is no longer needed

## Configuration

### Configuration File

The application uses a configuration system with the following priority (highest to lowest):

1. Environment variables
2. Configuration file (`whisper_api.conf`)
3. Default constants

The configuration file uses TOML format and should be placed in the same directory as the application. Here's an example of the configuration file:

```
# Whisper API Configuration File
WHISPER_API_HOST = "192.168.0.116"
WHISPER_API_PORT = "8181"
WHISPER_CMD = "/home/llm/whisper_api/whisperx.sh"
# Additional configuration options...
```

At startup, the application will:
1. Try to load the configuration from `whisper_api.conf`
2. Set environment variables from the config file if they don't already exist
3. Use environment variables or fall back to default values

This allows for flexible configuration management across different environments.

### Environment Variables

The application can be configured using the following environment variables:

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `WHISPER_CMD` | Path to the WhisperX script | `/home/llm/whisper_api/whisperx.sh` |
| `WHISPER_MODELS_DIR` | Path to the models directory | `/models` |
| `WHISPER_OUTPUT_DIR` | Directory for WhisperX to store output files | `/home/llm/whisper_api/output` |
| `WHISPER_OUTPUT_FORMAT` | Default output format for transcriptions | `txt` |
| `WHISPER_TMP_FILES` | Directory for storing temporary files | `/home/llm/whisper_api/tmp` |
| `WHISPER_API_HOST` | Host address to bind the server | `127.0.0.1` |
| `WHISPER_API_PORT` | Port for the HTTP server | `8181` |
| `WHISPER_API_TIMEOUT` | Client disconnect timeout in seconds | `480` |
| `WHISPER_API_KEEPALIVE` | Keep-alive timeout in seconds | `480` |
| `HTTP_WORKER_NUMBER` | Number of HTTP worker processes (0 = use number of CPU cores) | `0` |
| `WHISPER_JOB_RETENTION_HOURS` | Number of hours to keep job files before automatic cleanup | `48` |
| `WHISPER_CLEANUP_INTERVAL_HOURS` | Interval in hours between cleanup runs | `1` |
| `MAX_FILE_SIZE` | Maximum file size for uploads in bytes | `536870912` (512MB) |
| `ENABLE_CONCURRENCY` | Enable concurrent job processing | `false` |
| `MAX_CONCURRENT_JOBS` | Maximum number of concurrent jobs (when enabled) | `6` |
| `ENABLE_AUTHORIZATION` | Require authentication for all API requests | `true` |
| `SYNC_REQUEST_TIMEOUT_SECONDS` | Timeout for synchronous transcription requests (0 = no timeout) | `1800` |
| `DEFAULT_SYNC_MODE` | Default mode when sync parameter is missing (true = sync, false = async) | `false` |
| `RUST_LOG` | Logging level (error, warn, info, debug, trace) | `info` |
| `HF_TOKEN` | Hugging Face API token for diarization models access (can alternatively be passed per-request or loaded from file) | None |
| `WHISPER_HF_TOKEN_FILE` | Path to file containing Hugging Face API token | `/home/llm/whisper_api/hf_token.txt` |

## Authentication

By default, all API requests must include an Authorization header with a Bearer token:

```
Authorization: Bearer your_token_here
```

Requests without a valid Authorization header will be rejected with a 401 Unauthorized response.

Note: Currently, the API uses a dummy verification that accepts any token, but the header must be present and properly formatted.

Authentication can be disabled by setting `ENABLE_AUTHORIZATION=false` in the configuration file or environment variables. When disabled, requests will be accepted without any Authorization header.

## API Endpoints

### Submit a Transcription Job

```
POST /transcription
```

**Form Parameters**:
- `file`: Audio file to transcribe (required)
- `language`: Language code (optional, default: "fr")
- `model`: Model name (optional, default: "large-v3")
- `diarize`: Whether to apply speaker diarization (optional, default: true)
- `prompt`: Initial text prompt for transcription (optional, default: "")
- `hf_token`: Hugging Face API token for accessing diarization models (optional, if not provided will try to load from `hf_token.txt` file). Required for speaker diarization to work properly.
- `response_format`: Format of transcription output (optional, values: "srt", "vtt", "txt", "tsv", "json", "aud", default: "txt")
- `sync`: Whether to process the request synchronously (optional, values: "true", "false", default: value of `DEFAULT_SYNC_MODE` configuration)

**Response (Async Mode - default)**:
```json
{
  "job_id": "uuid-string",
  "status_url": "/transcription/uuid-string"
}
```

**Response (Sync Mode - when sync=true)**:
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

### Check Transcription Status

```
GET /transcription/{job_id}
```

**Response**:
```json
{
  "status": "Queued|Processing|Completed|Failed",
  "queue_position": 1,
  "data": "Additional information (result or error)"
}
```

Notes:
- The `queue_position` field is only included when the status is "Queued".
- It indicates the job's position in the queue (1-based index), where 1 means it's the next job to be processed after the current one.
- If a job is already processing or completed, the `queue_position` field will not be included in the response.
- You can use this value to provide users with an estimated wait time or position in line.

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

**Security Note**: When this endpoint is called, the system automatically removes all transcription files (both from the temporary directory and the WhisperX output directory) to ensure privacy and prevent data leakage.

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

## Configuration Example

A complete `whisper_api.conf` file might look like this:

```
# Server Configuration 
WHISPER_API_HOST = "0.0.0.0"
WHISPER_API_PORT = "9000"
WHISPER_API_TIMEOUT = 600
WHISPER_API_KEEPALIVE = 600
HTTP_WORKER_NUMBER = 4  # Use 4 workers (limited to number of CPU cores)

# API Configuration
SYNC_REQUEST_TIMEOUT_SECONDS = 1800  # 30 minutes timeout for synchronous requests
DEFAULT_SYNC_MODE = false            # Default to asynchronous mode when 'sync' parameter is missing

# File Storage Configuration
WHISPER_TMP_FILES = "/data/whisper/tmp"
WHISPER_HF_TOKEN_FILE = "/data/secrets/hf_token.txt"

# WhisperX Configuration
WHISPER_CMD = "/opt/whisper/whisperx.sh"
WHISPER_MODELS_DIR = "/opt/whisper/models"
WHISPER_OUTPUT_DIR = "/data/whisper/output"
WHISPER_OUTPUT_FORMAT = "srt"

# Job Management Configuration
WHISPER_JOB_RETENTION_HOURS = 24
WHISPER_CLEANUP_INTERVAL_HOURS = 6

# Upload Configuration
MAX_FILE_SIZE = 1073741824  # 1GB

# Concurrency Configuration
ENABLE_CONCURRENCY = true
MAX_CONCURRENT_JOBS = 6

# Security Configuration
ENABLE_AUTHORIZATION = true
```

## Test Commands

### Submit Transcription (Asynchronous Mode - Default)
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "language=fr" \
  -F "diarize=true" \
  -F "prompt=Meeting transcript:" \
  -F "hf_token=YOUR_HUGGINGFACE_TOKEN" \
  -F "response_format=txt" \
  -F "file=@/path/to/audio.wav"
```

### Submit Transcription (Synchronous Mode)
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "language=fr" \
  -F "diarize=true" \
  -F "prompt=Meeting transcript:" \
  -F "hf_token=YOUR_HUGGINGFACE_TOKEN" \
  -F "sync=true" \
  -F "file=@/path/to/audio.wav"
```

### Check Status (with Queue Position)
```bash
curl -X GET "http://localhost:8181/transcription/YOUR_JOB_ID" \
  -H "Authorization: Bearer your_token_here"
```

Note: 
- The maximum file size is configurable using the `MAX_FILE_SIZE` setting (default: 512 MB).
- Job processing can be configured for concurrent operation by setting `ENABLE_CONCURRENCY=true` and `MAX_CONCURRENT_JOBS` to the desired number of simultaneous jobs (default: 6).
- When concurrency is enabled, the queue position reflects the "batch" in which a job will be processed rather than its exact position in the queue.
- The number of HTTP workers is configurable using the `HTTP_WORKER_NUMBER` setting (0 = use number of CPU cores, >0 = use specified number up to the number of CPU cores).
- The API supports both synchronous and asynchronous transcription modes:
  - **Asynchronous Mode** (default): Returns immediately with a job ID and requires polling for status/results
  - **Synchronous Mode**: Waits for transcription to complete and returns the result directly (use `sync=true` parameter)
  - Default mode when `sync` parameter is missing is controlled by `DEFAULT_SYNC_MODE` setting (default: false = async)
  - Timeout for synchronous requests is configurable with `SYNC_REQUEST_TIMEOUT_SECONDS` (default: 1800 seconds, 0 = no timeout)

### Examples

#### Check Job Status and Queue Position
```bash
curl -X GET "http://localhost:8181/transcription/123e4567-e89b-12d3-a456-426614174000" \
  -H "Authorization: Bearer your_token_here"
```

Example response for a queued job:
```json
{
  "status": "Queued",
  "queue_position": 3
}
```

Example response for a processing job:
```json
{
  "status": "Processing"
}
```

#### Disable Speaker Diarization
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "diarize=false" \
  -F "file=@/path/to/audio.wav"
```

#### Add Initial Prompt
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "prompt=This is an interview between John and Sarah:" \
  -F "file=@/path/to/audio.wav"
```

#### Use Diarization with Hugging Face Token
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "diarize=true" \
  -F "hf_token=YOUR_HUGGINGFACE_TOKEN" \
  -F "file=@/path/to/audio.wav"
```

Note: If you don't provide the `hf_token` parameter, the system will attempt to read the token from the file specified by `WHISPER_HF_TOKEN_FILE` (default: `/home/llm/whisper_api/hf_token.txt`). If no token is available (file missing, empty, or unreadable) and diarization is requested, diarization will be automatically disabled with a warning in the logs. This ensures transcription jobs can proceed even without a token, but without speaker identification.

#### Specify Output Format
```bash
curl -X POST "http://localhost:8181/transcription" \
  -H "Authorization: Bearer your_token_here" \
  -F "response_format=srt" \
  -F "file=@/path/to/audio.wav"
```

## File Structure

The API is organized into modular components:

- **handlers/**: HTTP request handlers and form processing
- **models.rs**: Data structures for requests and responses
- **file_utils.rs**: File operations and resource management
- **queue_manager.rs**: Job queue and transcription processing with secure file cleanup
- **config.rs**: Application configuration management
- **config_loader.rs**: Configuration loading from TOML file and environment variables
- **error.rs**: Error handling and HTTP responses
- **whisperx.sh**: Wrapper script for running WhisperX in its virtual environment

The `whisperx.sh` script is responsible for activating the Python virtual environment, running the WhisperX command with the provided arguments, and then deactivating the environment. This ensures proper execution of WhisperX without requiring the API to manage Python environments directly.

The path to this script can be configured via the `WHISPER_CMD` setting in the configuration file or environment variable.

## API and Processing Models

The Whisper API supports multiple modes of operation:

### Request Processing Modes

1. **Asynchronous Mode** (default)
   - Returns immediately with a job ID
   - Client must poll for status and retrieve results separately 
   - Doesn't block HTTP connections during processing
   - Ideal for long transcriptions or when immediate results aren't needed

2. **Synchronous Mode**
   - Waits for the transcription to complete before responding
   - Returns transcription results directly in the response
   - Simpler client implementation (single HTTP request)
   - Configurable timeout with `SYNC_REQUEST_TIMEOUT_SECONDS`
   - Use by adding `sync=true` parameter to `/transcription` requests

### Job Processing Models

The Whisper API supports two processing models that can be configured via the `ENABLE_CONCURRENCY` setting:

1. **Sequential Processing** (default, `ENABLE_CONCURRENCY=false`):
   - Jobs are processed one at a time in FIFO order
   - Prevents GPU memory contention by ensuring only one transcription runs at a time
   - Simplifies resource management and provides predictable processing
   - Queue position directly indicates how many jobs are ahead in the queue
   - Recommended for systems with limited GPU memory

2. **Concurrent Processing** (`ENABLE_CONCURRENCY=true`):
   - Multiple jobs can be processed simultaneously
   - The number of concurrent jobs is controlled by the `MAX_CONCURRENT_JOBS` setting (default: 6)
   - Increases throughput when sufficient GPU memory is available
   - Queue positions are reported in "batches" based on concurrency level
   - Optimal for systems with multiple GPUs or high-memory GPUs

## Security and Privacy

The Whisper API implements several security and privacy measures:

1. **Authentication**: All API endpoints require Bearer token authentication
2. **Comprehensive File Cleanup**:
   - Temporary files in the job directory are removed after delivering results
   - Output files in the WhisperX output directory are also removed
   - Files are deleted immediately after successful result delivery
   - Automatic cleanup of expired jobs runs periodically
3. **File Isolation**:
   - Each job gets a unique UUID-based directory
   - Audio files and transcription results are isolated
   - File paths are not exposed to clients

## Resources

- WhisperX: https://github.com/m-bain/whisperX
- [WhisperX Documentation](whisperx.md)
