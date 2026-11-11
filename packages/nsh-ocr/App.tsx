import { mount, Text, View } from "nsh";

/** A placeholder screen: the recognizers run through the Rust crate, not this demo. */
function App() {
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="title" class="text-foreground">nsh-ocr</Text>
      <Text testID="copy" class="text-foreground">
        Recognise text in images with the platform's engine, through the Rust crate.
      </Text>
    </View>
  );
}

mount(App);
