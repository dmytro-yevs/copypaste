Pod::Spec.new do |spec|
  spec.name = 'hotkey_manager_macos'
  spec.version = '0.2.0'
  spec.summary = 'macOS implementation of the hotkey_manager Flutter plugin.'
  spec.description = <<-DESC
macOS implementation of the hotkey_manager Flutter plugin.
                       DESC
  spec.homepage = 'https://github.com/leanflutter/hotkey_manager'
  spec.license = { :file => '../LICENSE' }
  spec.author = { 'LiJianying' => 'lijy91@foxmail.com' }

  spec.source = { :path => '.' }
  spec.source_files = 'hotkey_manager_macos/Classes/**/*'
  spec.dependency 'FlutterMacOS'
  spec.dependency 'HotKey'

  spec.platform = :osx, '10.15'
  spec.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  spec.swift_version = '5.0'
end
