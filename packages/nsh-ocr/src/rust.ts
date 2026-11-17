import { NativeTextRecognizer, type NativeImageInput, type NativeRecognitionOptions } from "nsh-ocr/native";
import type { OcrBackend } from "./backend";
import { asOcrError } from "./errors";
import type { ImageInput } from "./input";
import type { RecognitionOptions } from "./options";
import type { RecognizedText } from "./text";

/**
 * The Rust crate's recognition (`nsh-ocr/native`). The app or package must list the crate under
 * `"rust"` in `nsh.json`; importing this module without it fails the bundle.
 */
export function rustBackend(): OcrBackend {
  return new RustBackend();
}

/**
 * Supplies the ocrs engine's model bytes (Linux and the web), parsed now so broken bytes
 * fail here. Targets whose engine needs no models reject with `ERR_OCR_NOT_AVAILABLE`.
 */
export async function useOcrsModels(detection: Uint8Array, recognition: Uint8Array): Promise<void> {
  try {
    await NativeTextRecognizer.useOcrsModels(detection, recognition);
  } catch (error) {
    throw asOcrError(error);
  }
}

function nativeImage(image: ImageInput): NativeImageInput {
  if ("path" in image) return { path: image.path };
  if ("encoded" in image) return { encoded: image.encoded };
  return { rgba: { width: image.width, height: image.height, data: image.data } };
}

/** Only the options the crate reads; unset fields keep the crate's defaults. */
function nativeOptions(options: RecognitionOptions): NativeRecognitionOptions {
  const result: NativeRecognitionOptions = {};
  if (options.script !== undefined) result.script = options.script;
  if (options.timeoutMs !== undefined) result.timeoutMs = options.timeoutMs;
  return result;
}

class RustBackend implements OcrBackend {
  private readonly native = new NativeTextRecognizer();

  supportedLanguages(): Promise<string[]> {
    return this.settle(() => this.native.supportedLanguages());
  }

  processImage(image: ImageInput, options: RecognitionOptions): Promise<RecognizedText> {
    return this.settle(async () => {
      // The package's only cast: the crate emits these camelCase shapes by contract.
      return (await this.native.processImage(nativeImage(image), nativeOptions(options))) as RecognizedText;
    });
  }

  cancel(): Promise<boolean> {
    return this.settle(() => this.native.cancel());
  }

  close(): Promise<void> {
    return this.settle(() => this.native.close());
  }

  private async settle<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      throw asOcrError(error);
    }
  }
}
