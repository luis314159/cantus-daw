// The JDK's own XML Schema validator, for the ignored test `the_jdk_validator_agrees` in
// `crates/cantus-project/tests/dawproject.rs`: java Validate.java <schema.xsd> <file.xml>
import javax.xml.XMLConstants;
import javax.xml.transform.stream.StreamSource;
import javax.xml.validation.SchemaFactory;

public class Validate {
    public static void main(String[] a) throws Exception {
        SchemaFactory f = SchemaFactory.newInstance(XMLConstants.W3C_XML_SCHEMA_NS_URI);
        f.newSchema(new java.io.File(a[0])).newValidator().validate(new StreamSource(new java.io.File(a[1])));
        System.out.println("valid " + a[1]);
    }
}
