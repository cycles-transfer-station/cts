import 'dart:io';
import 'dart:typed_data';
import 'dart:convert';
import 'package:ic_tools/ic_tools.dart';
import 'package:ic_tools/common.dart';
import 'package:ic_tools/candid.dart';
import 'package:ic_tools/tools.dart';
import 'package:crypto/crypto.dart';
import 'package:collection/collection.dart';





Future<void> main() async {
    
    print('testing ...');

    icbaseurl = Uri.parse('http://127.0.0.1:4943');
    Map ic_status_map = await ic_status();
    icrootkey = Uint8List.fromList(ic_status_map['root_key']!);
    
    
    
    
}
