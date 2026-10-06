import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/controller/module_form_draft.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';

void main() {
  test(
    'selected files remain available until the invocation draft closes',
    () async {
      const image = ModuleField(
        id: 'image_path',
        title: 'Image',
        kind: ModuleFieldKind.file,
        defaultValue: '',
        required: true,
        acceptedExtensions: ['png'],
      );
      final picker = _Picker();
      final draft = ModuleFormDraft(
        fields: [image],
        initial: {},
        picker: picker,
      );
      expect(draft.valid, isFalse);
      await draft.chooseFile(image);
      expect(draft.valid, isTrue);
      expect(draft.fileName('image_path'), 'image.png');
      expect(draft.values['image_path'], '/private/image.png');
      expect(picker.disposed, 0);
      await draft.close();
      expect(picker.disposed, 1);
      await draft.close();
      expect(picker.disposed, 1);
    },
  );
}

class _Picker implements ModuleInputPicker {
  int disposed = 0;
  @override
  Future<SelectedModuleInput?> chooseInput(ModuleField field) async =>
      SelectedModuleInput(
        path: '/private/image.png',
        name: 'image.png',
        dispose: () async {
          disposed++;
        },
      );
}
