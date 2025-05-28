WHISPER_API


This is a frontend for the Whisper API.
Its architecture is the following:
- the http handler that receives a request, pass the audio file to the queue manager and responds immediately to the client with an URL to the transcription when it is ready.
- the queue manager that receives the audio file, stores it in a temporary directory and manages the queue of requests : it passes the next file to the WhisperX command and stores the result in a temporary directory, then it invokes the http handler to notify the client that the transcription is ready.

- when the client requests the forged URL, either the http handler can respond that the transcription is not ready yet or it can respond by serving the transcription file. In this case, the http handler invokes the queue manager so this one can delete both the audio and transcription files.

In summary, the queue manager is responsible for: storing audio files in a temporary directory, managing the request queue by processing files one by one, executing the WhisperX command for transcription, storing the resulting transcriptions, and cleaning up both audio and transcription files once they have been successfully served to clients.
